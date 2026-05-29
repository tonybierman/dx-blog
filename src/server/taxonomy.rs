//! Category & tag read endpoints.

use dioxus::prelude::*;

use crate::model::{Category, Tag};

#[cfg(feature = "server")]
use crate::server::{sfe, DbExtension};

#[get("/api/categories", db: DbExtension)]
pub async fn list_categories() -> Result<Vec<Category>> {
    let rows = sqlx::query_as::<_, Category>(
        "SELECT id, name, slug, description FROM categories ORDER BY name",
    )
    .fetch_all(&db.0)
    .await
    .map_err(sfe)?;
    Ok(rows)
}

#[get("/api/tags", db: DbExtension)]
pub async fn list_tags() -> Result<Vec<Tag>> {
    let rows = sqlx::query_as::<_, Tag>("SELECT id, name, slug FROM tags ORDER BY name")
        .fetch_all(&db.0)
        .await
        .map_err(sfe)?;
    Ok(rows)
}

/// Look up a category by slug (for the category feed header).
#[post("/api/category", db: DbExtension)]
pub async fn get_category(slug: String) -> Result<Option<Category>> {
    let row = sqlx::query_as::<_, Category>(
        "SELECT id, name, slug, description FROM categories WHERE slug = ?",
    )
    .bind(&slug)
    .fetch_optional(&db.0)
    .await
    .map_err(sfe)?;
    Ok(row)
}

/// Look up a tag by slug (for the tag feed header).
#[post("/api/tag", db: DbExtension)]
pub async fn get_tag(slug: String) -> Result<Option<Tag>> {
    let row = sqlx::query_as::<_, Tag>("SELECT id, name, slug FROM tags WHERE slug = ?")
        .bind(&slug)
        .fetch_optional(&db.0)
        .await
        .map_err(sfe)?;
    Ok(row)
}
