//! Comment read/create endpoints. Moderation lives in `server::admin`.

use dioxus::prelude::*;

use crate::model::CommentView;

#[cfg(feature = "server")]
use crate::server::{sfe, DbExtension};

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

    let user = auth
        .current_user
        .as_ref()
        .filter(|u| !u.anonymous)
        .map(|u| u.id as i64);

    let (author_id, gname, gemail) = match user {
        Some(uid) => (Some(uid), None, None),
        None => {
            let name = guest_name.unwrap_or_default();
            let email = guest_email.unwrap_or_default();
            if name.trim().is_empty() || email.trim().is_empty() {
                return Err(
                    ServerFnError::new("Guests must provide a name and email.").into(),
                );
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
