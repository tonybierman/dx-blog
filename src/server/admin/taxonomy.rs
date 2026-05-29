//! Category & tag CRUD. Renames keep the slug stable so existing
//! `/category/:slug` and `/tag/:slug` links keep resolving.

use dioxus::prelude::*;

use crate::model::{Category, Tag};

#[cfg(feature = "server")]
use crate::auth_tokens::SETTINGS_WRITE;
#[cfg(feature = "server")]
use crate::server::{create_with_unique_slug, require_perm, sfe, DbExtension};

#[post("/api/admin/categories/create", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn create_category(name: String, description: Option<String>) -> Result<Category> {
    require_perm(&auth, SETTINGS_WRITE)?;
    let pool = &db.0;
    let (name_ref, description_ref) = (&name, &description);
    let (id, slug): (i64, String) =
        create_with_unique_slug(pool, "categories", &name, |slug| async move {
            let id = sqlx::query_scalar::<_, i64>(
                "INSERT INTO categories (name, slug, description) VALUES (?, ?, ?) RETURNING id",
            )
            .bind(name_ref)
            .bind(&slug)
            .bind(description_ref)
            .fetch_one(pool)
            .await?;
            Ok((id, slug))
        })
        .await?;
    Ok(Category {
        id,
        name,
        slug,
        description,
    })
}

#[post("/api/admin/categories/delete", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn delete_category(id: i64) -> Result<()> {
    require_perm(&auth, SETTINGS_WRITE)?;
    // Detach posts in this category so their `category_id` doesn't dangle. New
    // databases get this for free from the FK's ON DELETE SET NULL, but ones
    // created before that constraint was added rely on this explicit update.
    sqlx::query("UPDATE posts SET category_id = NULL WHERE category_id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    sqlx::query("DELETE FROM categories WHERE id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}

/// Rename a category. The slug is kept stable so existing `/category/:slug`
/// links keep resolving; only the display name changes.
#[post("/api/admin/categories/rename", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn rename_category(id: i64, name: String) -> Result<()> {
    require_perm(&auth, SETTINGS_WRITE)?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ServerFnError::new("Name can't be empty.").into());
    }
    sqlx::query("UPDATE categories SET name = ? WHERE id = ?")
        .bind(&name)
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}

#[post("/api/admin/tags/create", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn create_tag(name: String) -> Result<Tag> {
    require_perm(&auth, SETTINGS_WRITE)?;
    let pool = &db.0;
    let name_ref = &name;
    let (id, slug): (i64, String) =
        create_with_unique_slug(pool, "tags", &name, |slug| async move {
            let id = sqlx::query_scalar::<_, i64>(
                "INSERT INTO tags (name, slug) VALUES (?, ?) RETURNING id",
            )
            .bind(name_ref)
            .bind(&slug)
            .fetch_one(pool)
            .await?;
            Ok((id, slug))
        })
        .await?;
    Ok(Tag { id, name, slug })
}

#[post("/api/admin/tags/delete", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn delete_tag(id: i64) -> Result<()> {
    require_perm(&auth, SETTINGS_WRITE)?;
    sqlx::query("DELETE FROM post_tags WHERE tag_id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    sqlx::query("DELETE FROM tags WHERE id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}

/// Rename a tag. As with categories, the slug stays put so existing
/// `/tag/:slug` links keep working.
#[post("/api/admin/tags/rename", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn rename_tag(id: i64, name: String) -> Result<()> {
    require_perm(&auth, SETTINGS_WRITE)?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ServerFnError::new("Name can't be empty.").into());
    }
    sqlx::query("UPDATE tags SET name = ? WHERE id = ?")
        .bind(&name)
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}
