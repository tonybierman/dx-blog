//! Category & tag CRUD. Renames keep the slug stable so existing
//! `/category/:slug` and `/tag/:slug` links keep resolving.

use dioxus::prelude::*;

use crate::model::{Category, Tag};

#[cfg(feature = "server")]
use crate::auth_tokens::SETTINGS_WRITE;
#[cfg(feature = "server")]
use crate::db::taxonomy::{
    delete_category_db, delete_tag_db, insert_category_db, insert_tag_db, rename_category_db,
    rename_tag_db,
};
#[cfg(feature = "server")]
use crate::server::{create_with_unique_slug, require_perm, sfe, DbExtension};

#[post("/api/admin/categories/create", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn create_category(name: String, description: Option<String>) -> Result<Category> {
    require_perm(&auth, SETTINGS_WRITE)?;
    let pool = &db.0;
    let (name_ref, description_ref) = (&name, &description);
    let (id, slug): (i64, String) =
        create_with_unique_slug(pool, "categories", &name, |slug| async move {
            let id = insert_category_db(pool, name_ref, &slug, description_ref.as_deref()).await?;
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
    Ok(delete_category_db(&db.0, id).await.map_err(sfe)?)
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
    Ok(rename_category_db(&db.0, id, &name).await.map_err(sfe)?)
}

#[post("/api/admin/tags/create", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn create_tag(name: String) -> Result<Tag> {
    require_perm(&auth, SETTINGS_WRITE)?;
    let pool = &db.0;
    let name_ref = &name;
    let (id, slug): (i64, String) =
        create_with_unique_slug(pool, "tags", &name, |slug| async move {
            let id = insert_tag_db(pool, name_ref, &slug).await?;
            Ok((id, slug))
        })
        .await?;
    Ok(Tag { id, name, slug })
}

#[post("/api/admin/tags/delete", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn delete_tag(id: i64) -> Result<()> {
    require_perm(&auth, SETTINGS_WRITE)?;
    Ok(delete_tag_db(&db.0, id).await.map_err(sfe)?)
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
    Ok(rename_tag_db(&db.0, id, &name).await.map_err(sfe)?)
}
