//! Author profile read endpoint.

use dioxus::prelude::*;

use crate::model::AuthorProfile;

#[cfg(feature = "server")]
use crate::db::authors::get_author_profile_db;
#[cfg(feature = "server")]
use crate::server::{sfe, DbExtension};

#[post("/api/author", db: DbExtension)]
pub async fn get_author_profile(username: String) -> Result<Option<AuthorProfile>> {
    Ok(get_author_profile_db(&db.0, &username).await.map_err(sfe)?)
}
