//! Author profile read endpoint.

use dioxus::prelude::*;

use crate::model::AuthorProfile;

#[cfg(feature = "server")]
use crate::server::{sfe, DbExtension};

#[post("/api/author", db: DbExtension)]
pub async fn get_author_profile(username: String) -> Result<Option<AuthorProfile>> {
    let row = sqlx::query_as::<_, AuthorProfile>(
        r#"
        SELECT u.id AS user_id, u.username,
               COALESCE(u.display_name, u.username) AS display_name,
               u.avatar_url, up.bio, up.social_links
        FROM users u
        LEFT JOIN user_profiles up ON up.user_id = u.id
        WHERE u.username = ?
        "#,
    )
    .bind(&username)
    .fetch_optional(&db.0)
    .await
    .map_err(sfe)?;
    Ok(row)
}
