//! Comment read/create endpoints. Moderation lives in `server::admin`.

use dioxus::prelude::*;

use crate::model::{CommentView, RecentComment};

#[cfg(feature = "server")]
use crate::server::{live::HubExtension, looks_like_email, sfe, DbExtension};

/// Upper bounds on guest-supplied comment fields. The body is generous (a long
/// comment is legitimate); name/email are short. Without caps an unauthenticated
/// caller could store arbitrarily large blobs.
#[cfg(feature = "server")]
const MAX_BODY_LEN: usize = 5_000;
#[cfg(feature = "server")]
const MAX_NAME_LEN: usize = 100;
#[cfg(feature = "server")]
const MAX_EMAIL_LEN: usize = 254;

/// Per-commenter cooldown: one identity (logged-in id, else guest email) can't
/// post to the same post more than once within this window.
#[cfg(feature = "server")]
const COMMENT_COOLDOWN: &str = "-30 seconds";
/// Burst window: the trailing span over which a single post's comments are
/// counted toward [`POST_BURST_MAX`]. Bounds a varied-identity flood on one post
/// without a CAPTCHA; generous enough for legitimate activity.
#[cfg(feature = "server")]
const POST_BURST_WINDOW: &str = "-60 seconds";
/// Max comments any one post may accrue within [`POST_BURST_WINDOW`] before
/// further comments are refused.
#[cfg(feature = "server")]
const POST_BURST_MAX: i64 = 10;

/// The `CommentView` projection + its author join, shared by the public
/// per-post list here and the admin moderation queue in `server::admin`. Each
/// caller appends its own `WHERE`/`ORDER BY`. Centralized so the COALESCE
/// display-name fallback stays identical everywhere a `CommentView` is read.
#[cfg(feature = "server")]
pub(crate) const COMMENT_VIEW_COLUMNS: &str = "cm.id, cm.post_id, \
     COALESCE(u.display_name, u.username, cm.guest_name, 'Anonymous') AS display_name, \
     cm.body, cm.status, cm.created_at";
#[cfg(feature = "server")]
pub(crate) const COMMENT_VIEW_FROM: &str =
    "FROM comments cm LEFT JOIN users u ON u.id = cm.author_id";

/// The most recent approved comments across all published posts — backs the
/// home "Recent comments" sidebar. Public.
#[post("/api/comments/recent", db: DbExtension)]
pub async fn recent_comments(limit: i64) -> Result<Vec<RecentComment>> {
    let limit = limit.clamp(1, 10);
    let rows = sqlx::query_as::<_, RecentComment>(
        r#"
        SELECT cm.id, p.title AS post_title, p.slug AS post_slug,
               COALESCE(u.display_name, u.username, cm.guest_name, 'Anonymous') AS display_name,
               cm.body, cm.created_at
        FROM comments cm
        JOIN posts p ON p.id = cm.post_id AND p.status = 'published'
        LEFT JOIN users u ON u.id = cm.author_id
        WHERE cm.status = 'approved'
        ORDER BY cm.created_at DESC
        LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(&db.0)
    .await
    .map_err(sfe)?;
    Ok(rows)
}

/// Approved comments for a post, oldest first (public view).
#[post("/api/comments", db: DbExtension)]
pub async fn list_comments(post_id: i64) -> Result<Vec<CommentView>> {
    let rows = sqlx::query_as::<_, CommentView>(&format!(
        "SELECT {COMMENT_VIEW_COLUMNS} {COMMENT_VIEW_FROM} \
         WHERE cm.post_id = ? AND cm.status = 'approved' \
         ORDER BY cm.created_at ASC"
    ))
    .bind(post_id)
    .fetch_all(&db.0)
    .await
    .map_err(sfe)?;
    Ok(rows)
}

/// Post a comment. Logged-in users are attributed; guests must give name+email.
/// Defaults to `pending`; auto-approves a logged-in user who already has an
/// approved comment.
///
/// Returns the created [`CommentView`] (real id + final status) so the caller can
/// reconcile its optimistic placeholder. An approved comment is also broadcast
/// over the post's live channel so other readers see it without a refetch;
/// pending comments are not (they aren't publicly visible until moderated).
#[post("/api/comments/create", auth: arium_dioxus::auth::Session, db: DbExtension, hub: HubExtension)]
pub async fn create_comment(
    post_id: i64,
    body: String,
    guest_name: Option<String>,
    guest_email: Option<String>,
) -> Result<CommentView> {
    let body = body.trim().to_string();
    if body.is_empty() {
        return Err(ServerFnError::new("Comment cannot be empty.").into());
    }
    if body.chars().count() > MAX_BODY_LEN {
        return Err(ServerFnError::new("Comment is too long.").into());
    }

    // Only accept comments on a post that actually exists and is published —
    // otherwise anyone could POST arbitrary `post_id`s (including drafts or
    // non-existent ids) and pile up rows attached to nothing visible.
    let post_ok: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM posts WHERE id = ? AND status = 'published')",
    )
    .bind(post_id)
    .fetch_one(&db.0)
    .await
    .map_err(sfe)?;
    if !post_ok {
        return Err(ServerFnError::new("Post not found.").into());
    }

    let user = auth
        .current_user
        .as_ref()
        .filter(|u| !u.anonymous)
        .map(|u| u.id as i64);

    let (author_id, gname, gemail) = match user {
        Some(uid) => (Some(uid), None, None),
        None => {
            let name = guest_name.unwrap_or_default().trim().to_string();
            let email = guest_email.unwrap_or_default().trim().to_lowercase();
            if name.is_empty() || email.is_empty() {
                return Err(ServerFnError::new("Guests must provide a name and email.").into());
            }
            if name.chars().count() > MAX_NAME_LEN || email.len() > MAX_EMAIL_LEN {
                return Err(ServerFnError::new("Name or email is too long.").into());
            }
            // Same sanity check the subscribe flow uses — reject the obvious junk
            // the old non-empty-only guard let through.
            if !looks_like_email(&email) {
                return Err(ServerFnError::new("Please enter a valid email address.").into());
            }
            (None, Some(name), Some(email))
        }
    };

    // Anti-flood throttle for the public comment form. The global per-IP limiter
    // (main.rs) is the outer guard; these two gates are per-post so they survive
    // behind a shared proxy IP: a short cooldown per commenter identity, plus a
    // burst cap on how fast one post can accrue comments from any identities.
    let recent_by_identity: Option<i64> = match (author_id, gemail.as_deref()) {
        (Some(uid), _) => sqlx::query_scalar(
            "SELECT 1 FROM comments WHERE post_id = ? AND author_id = ? \
             AND created_at >= datetime('now', ?) LIMIT 1",
        )
        .bind(post_id)
        .bind(uid)
        .bind(COMMENT_COOLDOWN)
        .fetch_optional(&db.0)
        .await
        .map_err(sfe)?,
        (None, Some(email)) => sqlx::query_scalar(
            "SELECT 1 FROM comments WHERE post_id = ? AND guest_email = ? \
             AND created_at >= datetime('now', ?) LIMIT 1",
        )
        .bind(post_id)
        .bind(email)
        .bind(COMMENT_COOLDOWN)
        .fetch_optional(&db.0)
        .await
        .map_err(sfe)?,
        _ => None,
    };
    if recent_by_identity.is_some() {
        return Err(
            ServerFnError::new("You're commenting too quickly — please wait a moment.").into(),
        );
    }

    let burst: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM comments WHERE post_id = ? AND created_at >= datetime('now', ?)",
    )
    .bind(post_id)
    .bind(POST_BURST_WINDOW)
    .fetch_one(&db.0)
    .await
    .map_err(sfe)?;
    if burst >= POST_BURST_MAX {
        return Err(ServerFnError::new(
            "This post is receiving too many comments right now — please try again shortly.",
        )
        .into());
    }

    // Auto-approve a returning logged-in commenter.
    let status = if let Some(uid) = author_id {
        let prior: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM comments WHERE author_id = ? AND status = 'approved' LIMIT 1",
        )
        .bind(uid)
        .fetch_optional(&db.0)
        .await
        .map_err(sfe)?;
        if prior.is_some() {
            "approved"
        } else {
            "pending"
        }
    } else {
        "pending"
    };

    let inserted = sqlx::query(
        "INSERT INTO comments (post_id, author_id, guest_name, guest_email, body, status)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(post_id)
    .bind(author_id)
    .bind(&gname)
    .bind(&gemail)
    .bind(&body)
    .bind(status)
    .execute(&db.0)
    .await
    .map_err(sfe)?;

    // Re-select the row through the shared projection so `display_name` and
    // `created_at` resolve exactly as `list_comments` would — the optimistic
    // client reconciles against this, so it must match what other readers see.
    let view = sqlx::query_as::<_, CommentView>(&format!(
        "SELECT {COMMENT_VIEW_COLUMNS} {COMMENT_VIEW_FROM} WHERE cm.id = ?"
    ))
    .bind(inserted.last_insert_rowid())
    .fetch_one(&db.0)
    .await
    .map_err(sfe)?;

    // Only approved comments are public, so only they go out live.
    if view.status == "approved" {
        hub.publish(post_id, crate::model::LiveEvent::Comment(view.clone()));
    }

    Ok(view)
}
