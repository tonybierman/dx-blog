//! Comment read/create endpoints. Moderation lives in `server::admin`.

use dioxus::prelude::*;

use crate::model::{CommentView, RecentComment};

#[cfg(feature = "server")]
use crate::server::{looks_like_email, sfe, DbExtension};

/// Upper bounds on guest-supplied comment fields. The body is generous (a long
/// comment is legitimate); name/email are short. Without caps an unauthenticated
/// caller could store arbitrarily large blobs.
#[cfg(feature = "server")]
const MAX_BODY_LEN: usize = 5_000;
#[cfg(feature = "server")]
const MAX_NAME_LEN: usize = 100;
#[cfg(feature = "server")]
const MAX_EMAIL_LEN: usize = 254;

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
    let rows = sqlx::query_as::<_, CommentView>(
        r#"
        SELECT cm.id, cm.post_id,
               COALESCE(u.display_name, u.username, cm.guest_name, 'Anonymous') AS display_name,
               cm.body, cm.status, cm.created_at
        FROM comments cm
        LEFT JOIN users u ON u.id = cm.author_id
        WHERE cm.post_id = ? AND cm.status = 'approved'
        ORDER BY cm.created_at ASC
        "#,
    )
    .bind(post_id)
    .fetch_all(&db.0)
    .await
    .map_err(sfe)?;
    Ok(rows)
}

/// Post a comment. Logged-in users are attributed; guests must give name+email.
/// Defaults to `pending`; auto-approves a logged-in user who already has an
/// approved comment.
#[post("/api/comments/create", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn create_comment(
    post_id: i64,
    body: String,
    guest_name: Option<String>,
    guest_email: Option<String>,
) -> Result<()> {
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

    sqlx::query(
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

    Ok(())
}
