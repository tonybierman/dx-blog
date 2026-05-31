//! Comment read/create endpoints. Moderation lives in `server::admin`.

use dioxus::prelude::*;

use crate::model::{CommentView, RecentComment};

#[cfg(feature = "server")]
use crate::db::comments::{
    burst_count_db, comment_with_post_db, commenter_cooldown_email_db, commenter_cooldown_uid_db,
    fetch_comment_by_id_db, has_prior_approved_comment_db, insert_comment_db, list_comments_db,
    post_is_published_db, recent_comments_db,
};
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

/// The most recent approved comments across all published posts — backs the
/// home "Recent comments" sidebar. Public.
#[post("/api/comments/recent", db: DbExtension)]
pub async fn recent_comments(limit: i64) -> Result<Vec<RecentComment>> {
    let limit = limit.clamp(1, 10);
    Ok(recent_comments_db(&db.0, limit).await.map_err(sfe)?)
}

/// Approved comments for a post, oldest first (public view).
#[post("/api/comments", db: DbExtension)]
pub async fn list_comments(post_id: i64) -> Result<Vec<CommentView>> {
    Ok(list_comments_db(&db.0, post_id).await.map_err(sfe)?)
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
    if !post_is_published_db(&db.0, post_id).await.map_err(sfe)? {
        return Err(ServerFnError::new("Post not found.").into());
    }

    let user = auth
        .current_user
        .as_ref()
        .filter(|u| !u.anonymous)
        .map(|u| u.id);

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
    let throttled = match (author_id, gemail.as_deref()) {
        (Some(uid), _) => commenter_cooldown_uid_db(&db.0, post_id, uid, COMMENT_COOLDOWN)
            .await
            .map_err(sfe)?
            .is_some(),
        (None, Some(email)) => commenter_cooldown_email_db(&db.0, post_id, email, COMMENT_COOLDOWN)
            .await
            .map_err(sfe)?
            .is_some(),
        _ => false,
    };
    if throttled {
        return Err(
            ServerFnError::new("You're commenting too quickly — please wait a moment.").into(),
        );
    }

    let burst = burst_count_db(&db.0, post_id, POST_BURST_WINDOW)
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
        if has_prior_approved_comment_db(&db.0, uid)
            .await
            .map_err(sfe)?
        {
            "approved"
        } else {
            "pending"
        }
    } else {
        "pending"
    };

    let row_id = insert_comment_db(
        &db.0,
        post_id,
        author_id,
        gname.as_deref(),
        gemail.as_deref(),
        &body,
        status,
    )
    .await
    .map_err(sfe)?;

    // Re-select the row through the shared projection so `display_name` and
    // `created_at` resolve exactly as `list_comments` would — the optimistic
    // client reconciles against this, so it must match what other readers see.
    let view = fetch_comment_by_id_db(&db.0, row_id).await.map_err(sfe)?;

    // Only approved comments are public, so only they go out live.
    if view.status == "approved" {
        hub.publish(post_id, crate::model::LiveEvent::Comment(view.clone()));
    }

    // Notify admins of EVERY new comment, pending or approved — this is the only
    // live signal for pending comments (they never touch the public per-post
    // channel). Best-effort: the comment is already stored, so a failed notice
    // lookup must not fail the request.
    if let Ok(c) = comment_with_post_db(&db.0, row_id).await {
        hub.publish_admin(crate::model::AdminEvent::Comment {
            id: c.id,
            post_id: c.post_id,
            post_title: c.post_title,
            post_slug: c.post_slug,
            display_name: c.display_name,
            status: c.status,
        });
    }

    Ok(view)
}
