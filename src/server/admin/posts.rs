//! Post authoring mutations and the edit-form read path.

use dioxus::prelude::*;

use crate::model::{PostCard, PostEditData};

#[cfg(feature = "server")]
use crate::auth_tokens::{POSTS_WRITE, POSTS_WRITE_ANY};
#[cfg(feature = "server")]
use crate::db::posts::{
    admin_list_posts_db, delete_post_db, get_post_edit_db, insert_post_db, set_post_tags_db,
    update_post_db,
};
#[cfg(feature = "server")]
use crate::server::{create_with_unique_slug, render_markdown, require_perm, sfe, DbExtension};

#[cfg(feature = "server")]
use super::can_edit_post;

// ---------------------------------------------------------------- validation helpers

/// Reject a `status` outside the lifecycle whitelist before it reaches the DB.
#[cfg(feature = "server")]
fn validate_status(status: &str) -> std::result::Result<(), ServerFnError> {
    if crate::model::POST_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(ServerFnError::new("Invalid status."))
    }
}

/// Validate a post's optional `featured_image_url` before it's stored.
#[cfg(feature = "server")]
fn validate_featured_image(url: &Option<String>) -> std::result::Result<(), ServerFnError> {
    let Some(u) = url.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    let same_origin_path = u.starts_with('/') && !u.starts_with("//");
    let http_url = matches!(u.split_once("://"), Some((scheme, rest))
        if (scheme == "http" || scheme == "https")
            && rest.split('/').next().is_some_and(|host| !host.is_empty() && !host.contains([' ', '\t'])));
    if same_origin_path || http_url {
        Ok(())
    } else {
        Err(ServerFnError::new(
            "Featured image must be a site path (starting with /) or an http(s) URL.",
        ))
    }
}

// ---------------------------------------------------------------- posts

/// Create a post. Requires the `posts:write` capability; the creator becomes
/// the post's resource Owner.
#[post("/api/posts/create", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn create_post(
    title: String,
    body_md: String,
    excerpt: String,
    category_id: Option<i64>,
    tag_ids: Vec<i64>,
    featured_image_url: Option<String>,
    status: String,
) -> Result<i64> {
    let uid = require_perm(&auth, POSTS_WRITE)?;
    validate_status(&status)?;
    validate_featured_image(&featured_image_url)?;
    let body_html = render_markdown(&body_md);

    let pool = &db.0;
    let (title_ref, body_md_ref, body_html_ref, excerpt_ref, image_ref, status_ref) = (
        &title,
        &body_md,
        &body_html,
        &excerpt,
        &featured_image_url,
        &status,
    );
    let post_id: i64 = create_with_unique_slug(pool, "posts", &title, |slug| async move {
        insert_post_db(
            pool,
            title_ref,
            &slug,
            body_md_ref,
            body_html_ref,
            excerpt_ref,
            uid,
            category_id,
            image_ref.as_deref(),
            status_ref,
        )
        .await
    })
    .await?;

    set_post_tags_db(&db.0, post_id, &tag_ids)
        .await
        .map_err(sfe)?;

    // Creator becomes Owner. Direct upsert bootstraps the membership (grant_membership
    // requires a pre-existing Manager, which a brand-new post has none of).
    sqlx::query(
        "INSERT INTO arium_resource_members (kind, resource_id, user_id, role)
         VALUES ('post', ?, ?, ?)
         ON CONFLICT (kind, resource_id, user_id) DO UPDATE SET role = excluded.role",
    )
    .bind(post_id)
    .bind(uid)
    .bind(arium_dioxus::ResourceRole::Owner.as_str())
    .execute(&db.0)
    .await
    .map_err(sfe)?;

    Ok(post_id)
}

/// Update an existing post (Editor on the post, or admin token).
#[post("/api/posts/update", auth: arium_dioxus::auth::Session, db: DbExtension, authority: arium_dioxus::ResourceAuthorityExt)]
pub async fn update_post(
    id: i64,
    title: String,
    body_md: String,
    excerpt: String,
    category_id: Option<i64>,
    tag_ids: Vec<i64>,
    featured_image_url: Option<String>,
    status: String,
) -> Result<()> {
    can_edit_post(&auth, &db.0, &authority, id).await?;
    validate_status(&status)?;
    validate_featured_image(&featured_image_url)?;
    let body_html = render_markdown(&body_md);

    update_post_db(
        &db.0,
        id,
        &title,
        &body_md,
        &body_html,
        &excerpt,
        category_id,
        featured_image_url.as_deref(),
        &status,
    )
    .await
    .map_err(sfe)?;

    set_post_tags_db(&db.0, id, &tag_ids).await.map_err(sfe)?;
    Ok(())
}

/// Delete a post (admin only).
#[post("/api/posts/delete", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn delete_post(id: i64) -> Result<()> {
    require_perm(&auth, POSTS_WRITE_ANY)?;
    Ok(delete_post_db(&db.0, id).await.map_err(sfe)?)
}

/// Posts visible to the current author/admin (admins see all; authors see own).
#[post("/api/admin/posts", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn admin_list_posts(
    status_filter: Option<String>,
    sort: Option<String>,
) -> Result<Vec<PostCard>> {
    let uid = require_perm(&auth, POSTS_WRITE)?;
    let is_admin = auth
        .current_user
        .as_ref()
        .map(|u| u.permissions.contains(POSTS_WRITE_ANY))
        .unwrap_or(false);
    let status_filter = status_filter.filter(|s| !s.is_empty());
    Ok(admin_list_posts_db(
        &db.0,
        is_admin,
        uid,
        status_filter.as_deref(),
        sort.as_deref(),
    )
    .await
    .map_err(sfe)?)
}

/// Raw fields for the edit form (Editor on the post, or admin token).
#[post("/api/admin/post-edit", auth: arium_dioxus::auth::Session, db: DbExtension, authority: arium_dioxus::ResourceAuthorityExt)]
pub async fn get_post_edit(id: i64) -> Result<PostEditData> {
    can_edit_post(&auth, &db.0, &authority, id).await?;
    Ok(get_post_edit_db(&db.0, id).await.map_err(sfe)?)
}
