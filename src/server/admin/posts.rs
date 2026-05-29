//! Post authoring mutations and the edit-form read path.

use dioxus::prelude::*;

use crate::model::{PostCard, PostEditData};

#[cfg(feature = "server")]
use crate::auth_tokens::{POSTS_WRITE, POSTS_WRITE_ANY};
#[cfg(feature = "server")]
use crate::server::{
    create_with_unique_slug, render_markdown, require_perm, sfe, DbExtension, POST_CARD_COLUMNS,
    POST_CARD_JOINS,
};

#[cfg(feature = "server")]
use super::can_edit_post;

// ---------------------------------------------------------------- validation helpers

/// Reject a `status` outside the lifecycle whitelist before it reaches the DB.
/// The `posts` table has a matching `CHECK`, but binding an invalid value would
/// surface a raw sqlx CHECK error to the client; this returns a friendly one.
#[cfg(feature = "server")]
fn validate_status(status: &str) -> std::result::Result<(), ServerFnError> {
    if crate::model::POST_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(ServerFnError::new("Invalid status."))
    }
}

/// Validate a post's optional `featured_image_url` before it's stored and later
/// rendered into `<img src>` and `og:image`. Accept only a same-origin relative
/// path (`/uploads/…`, our upload sink, or any site path) or an absolute http(s)
/// URL with a real host. This rejects `javascript:`/`data:`/`mailto:` schemes and
/// scheme-relative `//host` values — closing the residual SSRF-via-unfurler /
/// off-origin surface even though the value is gated behind `posts:write`.
#[cfg(feature = "server")]
fn validate_featured_image(url: &Option<String>) -> std::result::Result<(), ServerFnError> {
    let Some(u) = url.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(()); // empty / unset — no image, nothing to validate
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

    // Resolve+insert with a retry so a concurrent same-title create that grabs the
    // slug first lands us on the next suffix instead of a raw UNIQUE 500.
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
        sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO posts
              (title, slug, body_md, body_html, excerpt, author_id, category_id,
               featured_image_url, status, published_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?,
               CASE WHEN ? = 'published' THEN datetime('now') ELSE NULL END)
            RETURNING id
            "#,
        )
        .bind(title_ref)
        .bind(&slug)
        .bind(body_md_ref)
        .bind(body_html_ref)
        .bind(excerpt_ref)
        .bind(uid)
        .bind(category_id)
        .bind(image_ref)
        .bind(status_ref)
        .bind(status_ref)
        .fetch_one(pool)
        .await
    })
    .await?;

    set_post_tags(&db.0, post_id, &tag_ids).await?;

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

    sqlx::query(
        r#"
        UPDATE posts SET
          title = ?, body_md = ?, body_html = ?, excerpt = ?,
          category_id = ?, featured_image_url = ?, status = ?,
          published_at = CASE WHEN ? = 'published' AND published_at IS NULL
                              THEN datetime('now') ELSE published_at END,
          updated_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(&title)
    .bind(&body_md)
    .bind(&body_html)
    .bind(&excerpt)
    .bind(category_id)
    .bind(&featured_image_url)
    .bind(&status)
    .bind(&status)
    .bind(id)
    .execute(&db.0)
    .await
    .map_err(sfe)?;

    set_post_tags(&db.0, id, &tag_ids).await?;
    Ok(())
}

/// Delete a post (admin only).
#[post("/api/posts/delete", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn delete_post(id: i64) -> Result<()> {
    require_perm(&auth, POSTS_WRITE_ANY)?;
    // On a current DB the FK `ON DELETE CASCADE` clauses would clear the child
    // rows for us, but databases created before those constraints were added rely
    // on these explicit deletes (see the schema header). Run them in one
    // transaction so a mid-sequence failure can't leave a post half-deleted with
    // orphaned tags/comments/views/memberships. (arium_resource_members has no FK
    // to posts, so its rows are never cascaded — this is their only cleanup.)
    let mut tx = db.0.begin().await.map_err(sfe)?;
    for sql in [
        "DELETE FROM post_tags WHERE post_id = ?",
        "DELETE FROM comments WHERE post_id = ?",
        "DELETE FROM post_views WHERE post_id = ?",
        "DELETE FROM arium_resource_members WHERE kind = 'post' AND resource_id = ?",
        "DELETE FROM posts WHERE id = ?",
    ] {
        sqlx::query(sql)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(sfe)?;
    }
    tx.commit().await.map_err(sfe)?;
    Ok(())
}

/// Posts visible to the current author/admin (admins see all; authors see own),
/// optionally filtered by status and sorted. `status_filter` is `None`/empty for
/// all statuses; `sort` is one of a fixed whitelist (the ORDER BY clause is
/// chosen from constants, never interpolated from user input).
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

    let order_by = match sort.as_deref() {
        Some("title") => "p.title COLLATE NOCASE ASC, p.id DESC",
        Some("title_desc") => "p.title COLLATE NOCASE DESC, p.id DESC",
        Some("status") => "p.status ASC, p.updated_at DESC",
        Some("status_desc") => "p.status DESC, p.updated_at DESC",
        Some("published") => "p.published_at IS NULL, p.published_at DESC, p.id DESC",
        // Oldest-published first; unpublished (NULL) still sort last.
        Some("published_desc") => "p.published_at IS NULL, p.published_at ASC, p.id ASC",
        Some("oldest") => "p.updated_at ASC, p.id ASC",
        // "recent" / None / anything unrecognised
        _ => "p.updated_at DESC, p.id DESC",
    };

    let sql = format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         WHERE (? = 1 OR p.author_id = ?) AND (? IS NULL OR p.status = ?) \
         ORDER BY {order_by}"
    );

    let rows = sqlx::query_as::<_, PostCard>(&sql)
        .bind(is_admin as i64)
        .bind(uid)
        .bind(&status_filter)
        .bind(&status_filter)
        .fetch_all(&db.0)
        .await
        .map_err(sfe)?;
    Ok(rows)
}

/// Raw fields for the edit form (Editor on the post, or admin token).
#[post("/api/admin/post-edit", auth: arium_dioxus::auth::Session, db: DbExtension, authority: arium_dioxus::ResourceAuthorityExt)]
pub async fn get_post_edit(id: i64) -> Result<PostEditData> {
    can_edit_post(&auth, &db.0, &authority, id).await?;

    let row: (
        String,
        String,
        String,
        String,
        Option<i64>,
        Option<String>,
        String,
    ) = sqlx::query_as(
        "SELECT title, slug, body_md, excerpt, category_id, featured_image_url, status
             FROM posts WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&db.0)
    .await
    .map_err(sfe)?;

    let tag_ids: Vec<i64> = sqlx::query_scalar("SELECT tag_id FROM post_tags WHERE post_id = ?")
        .bind(id)
        .fetch_all(&db.0)
        .await
        .map_err(sfe)?;

    Ok(PostEditData {
        id,
        title: row.0,
        slug: row.1,
        body_md: row.2,
        excerpt: row.3,
        category_id: row.4,
        featured_image_url: row.5,
        status: row.6,
        tag_ids,
    })
}

#[cfg(feature = "server")]
async fn set_post_tags(
    pool: &arium_dioxus::pool::Pool,
    post_id: i64,
    tag_ids: &[i64],
) -> std::result::Result<(), ServerFnError> {
    sqlx::query("DELETE FROM post_tags WHERE post_id = ?")
        .bind(post_id)
        .execute(pool)
        .await
        .map_err(sfe)?;
    for tid in tag_ids {
        sqlx::query("INSERT OR IGNORE INTO post_tags (post_id, tag_id) VALUES (?, ?)")
            .bind(post_id)
            .bind(tid)
            .execute(pool)
            .await
            .map_err(sfe)?;
    }
    Ok(())
}

/// Render Markdown to sanitized HTML for the editor's live preview.
#[post("/api/admin/preview", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn preview_markdown(md: String) -> Result<String> {
    require_perm(&auth, POSTS_WRITE)?;
    let _ = &db; // pool unused; extractor kept for a uniform signature
    Ok(render_markdown(&md))
}
