//! Comment moderation mutations and the moderation-queue read path.

use dioxus::prelude::*;

use crate::model::CommentView;

#[cfg(feature = "server")]
use crate::auth_tokens::COMMENTS_MODERATE;
#[cfg(feature = "server")]
use crate::db::comments::{
    comment_with_post_db, delete_comment_db, fetch_comment_by_id_db, list_comments_admin_db,
    moderate_comment_db,
};
#[cfg(feature = "server")]
use crate::server::{live::HubExtension, require_perm, sfe, DbExtension};

/// Comments filtered by status (defaults to all) — moderation queue.
#[post("/api/admin/comments", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn admin_list_comments(status: Option<String>) -> Result<Vec<CommentView>> {
    require_perm(&auth, COMMENTS_MODERATE)?;
    Ok(list_comments_admin_db(&db.0, status.as_deref())
        .await
        .map_err(sfe)?)
}

#[post("/api/admin/comments/moderate", auth: arium_dioxus::auth::Session, db: DbExtension, hub: HubExtension)]
pub async fn moderate_comment(id: i64, status: String) -> Result<()> {
    require_perm(&auth, COMMENTS_MODERATE)?;
    if !crate::model::COMMENT_STATUSES.contains(&status.as_str()) {
        return Err(ServerFnError::new("Invalid status.").into());
    }
    moderate_comment_db(&db.0, id, &status).await.map_err(sfe)?;

    // Approving a comment is the moment it becomes public — push it to anyone
    // currently reading the post so it streams in without a refresh. This is the
    // common path for guest / first-time commenters whose comment started pending.
    if status == "approved" {
        let view = fetch_comment_by_id_db(&db.0, id).await.map_err(sfe)?;
        hub.publish(view.post_id, crate::model::LiveEvent::Comment(view));
    }

    // Notify admins of the new status on EVERY change (not just approval), so a
    // second open dashboard / moderation queue reflects an approve/reject live.
    if let Ok(c) = comment_with_post_db(&db.0, id).await {
        hub.publish_admin(crate::model::AdminEvent::Comment {
            id: c.id,
            post_id: c.post_id,
            post_title: c.post_title,
            post_slug: c.post_slug,
            display_name: c.display_name,
            status: c.status,
        });
    }
    Ok(())
}

#[post("/api/admin/comments/delete", auth: arium_dioxus::auth::Session, db: DbExtension, hub: HubExtension)]
pub async fn delete_comment(id: i64) -> Result<()> {
    require_perm(&auth, COMMENTS_MODERATE)?;
    delete_comment_db(&db.0, id).await.map_err(sfe)?;
    // Drop it from any open moderation view / dashboard feed.
    hub.publish_admin(crate::model::AdminEvent::CommentRemoved { id });
    Ok(())
}
