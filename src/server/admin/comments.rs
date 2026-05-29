//! Comment moderation mutations and the moderation-queue read path.

use dioxus::prelude::*;

use crate::model::CommentView;

#[cfg(feature = "server")]
use crate::auth_tokens::COMMENTS_MODERATE;
#[cfg(feature = "server")]
use crate::server::{require_perm, sfe, DbExtension};

/// Comments filtered by status (defaults to all) — moderation queue.
#[post("/api/admin/comments", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn admin_list_comments(status: Option<String>) -> Result<Vec<CommentView>> {
    require_perm(&auth, COMMENTS_MODERATE)?;
    use crate::server::comments::{COMMENT_VIEW_COLUMNS, COMMENT_VIEW_FROM};
    let rows = sqlx::query_as::<_, CommentView>(&format!(
        "SELECT {COMMENT_VIEW_COLUMNS} {COMMENT_VIEW_FROM} \
         WHERE (? IS NULL OR cm.status = ?) \
         ORDER BY cm.created_at DESC"
    ))
    .bind(&status)
    .bind(&status)
    .fetch_all(&db.0)
    .await
    .map_err(sfe)?;
    Ok(rows)
}

#[post("/api/admin/comments/moderate", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn moderate_comment(id: i64, status: String) -> Result<()> {
    require_perm(&auth, COMMENTS_MODERATE)?;
    if !crate::model::COMMENT_STATUSES.contains(&status.as_str()) {
        return Err(ServerFnError::new("Invalid status.").into());
    }
    sqlx::query("UPDATE comments SET status = ? WHERE id = ?")
        .bind(&status)
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}

#[post("/api/admin/comments/delete", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn delete_comment(id: i64) -> Result<()> {
    require_perm(&auth, COMMENTS_MODERATE)?;
    sqlx::query("DELETE FROM comments WHERE id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}
