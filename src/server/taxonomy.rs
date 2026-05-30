//! Category & tag read endpoints.

use dioxus::prelude::*;

use crate::model::{Category, Tag};

#[cfg(feature = "server")]
use crate::db::taxonomy::{get_category_db, get_tag_db, list_categories_db, list_tags_db};
#[cfg(feature = "server")]
use crate::server::{sfe, DbExtension};

#[get("/api/categories", db: DbExtension)]
pub async fn list_categories() -> Result<Vec<Category>> {
    Ok(list_categories_db(&db.0).await.map_err(sfe)?)
}

#[get("/api/tags", db: DbExtension)]
pub async fn list_tags() -> Result<Vec<Tag>> {
    Ok(list_tags_db(&db.0).await.map_err(sfe)?)
}

/// Look up a category by slug (for the category feed header).
#[post("/api/category", db: DbExtension)]
pub async fn get_category(slug: String) -> Result<Option<Category>> {
    Ok(get_category_db(&db.0, &slug).await.map_err(sfe)?)
}

/// Look up a tag by slug (for the tag feed header).
#[post("/api/tag", db: DbExtension)]
pub async fn get_tag(slug: String) -> Result<Option<Tag>> {
    Ok(get_tag_db(&db.0, &slug).await.map_err(sfe)?)
}
