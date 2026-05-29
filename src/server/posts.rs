//! Post read endpoints (public). Authoring/mutation lives in `server::admin`.

use dioxus::prelude::*;

use crate::model::{PostDetail, PostFeed, PER_PAGE};

#[cfg(feature = "server")]
use crate::server::{sfe, DbExtension};

/// Paginated published-post feed, optionally filtered by category or tag slug.
#[post("/api/posts", db: DbExtension)]
pub async fn list_posts(
    page: i64,
    category_slug: Option<String>,
    tag_slug: Option<String>,
) -> Result<PostFeed> {
    use crate::model::PostCard;
    let pool = &db.0;
    let page = page.max(1);
    let offset = (page - 1) * PER_PAGE;

    let items = sqlx::query_as::<_, PostCard>(
        r#"
        SELECT p.id, p.title, p.slug, p.excerpt, p.featured_image_url,
               p.author_id,
               COALESCE(u.display_name, u.username) AS author_name,
               c.name AS category_name,
               p.status, p.published_at
        FROM posts p
        JOIN users u ON u.id = p.author_id
        LEFT JOIN categories c ON c.id = p.category_id
        WHERE p.status = 'published'
          AND (? IS NULL OR c.slug = ?)
          AND (? IS NULL OR EXISTS (
                SELECT 1 FROM post_tags pt JOIN tags t ON t.id = pt.tag_id
                WHERE pt.post_id = p.id AND t.slug = ?))
        ORDER BY p.published_at DESC, p.id DESC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(&category_slug)
    .bind(&category_slug)
    .bind(&tag_slug)
    .bind(&tag_slug)
    .bind(PER_PAGE)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(sfe)?;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM posts p
        LEFT JOIN categories c ON c.id = p.category_id
        WHERE p.status = 'published'
          AND (? IS NULL OR c.slug = ?)
          AND (? IS NULL OR EXISTS (
                SELECT 1 FROM post_tags pt JOIN tags t ON t.id = pt.tag_id
                WHERE pt.post_id = p.id AND t.slug = ?))
        "#,
    )
    .bind(&category_slug)
    .bind(&category_slug)
    .bind(&tag_slug)
    .bind(&tag_slug)
    .fetch_one(pool)
    .await
    .map_err(sfe)?;

    Ok(PostFeed {
        items,
        total,
        page,
        per_page: PER_PAGE,
    })
}

/// All published posts, newest first — backs the Masonry archive.
#[get("/api/archive", db: DbExtension)]
pub async fn list_archive() -> Result<Vec<crate::model::PostCard>> {
    use crate::model::PostCard;
    let rows = sqlx::query_as::<_, PostCard>(
        r#"
        SELECT p.id, p.title, p.slug, p.excerpt, p.featured_image_url,
               p.author_id,
               COALESCE(u.display_name, u.username) AS author_name,
               c.name AS category_name,
               p.status, p.published_at
        FROM posts p
        JOIN users u ON u.id = p.author_id
        LEFT JOIN categories c ON c.id = p.category_id
        WHERE p.status = 'published'
        ORDER BY p.published_at DESC, p.id DESC
        "#,
    )
    .fetch_all(&db.0)
    .await
    .map_err(sfe)?;
    Ok(rows)
}

/// Published posts authored by a given username.
#[post("/api/author-posts", db: DbExtension)]
pub async fn posts_by_author(username: String) -> Result<Vec<crate::model::PostCard>> {
    use crate::model::PostCard;
    let rows = sqlx::query_as::<_, PostCard>(
        r#"
        SELECT p.id, p.title, p.slug, p.excerpt, p.featured_image_url,
               p.author_id,
               COALESCE(u.display_name, u.username) AS author_name,
               c.name AS category_name,
               p.status, p.published_at
        FROM posts p
        JOIN users u ON u.id = p.author_id
        LEFT JOIN categories c ON c.id = p.category_id
        WHERE p.status = 'published' AND u.username = ?
        ORDER BY p.published_at DESC, p.id DESC
        "#,
    )
    .bind(&username)
    .fetch_all(&db.0)
    .await
    .map_err(sfe)?;
    Ok(rows)
}

/// A single published post by slug, with author + category joined in.
#[post("/api/post", db: DbExtension)]
pub async fn get_post(slug: String) -> Result<Option<PostDetail>> {
    let post = sqlx::query_as::<_, PostDetail>(
        r#"
        SELECT p.id, p.title, p.slug, p.body_md, p.body_html, p.excerpt,
               p.featured_image_url, p.author_id,
               COALESCE(u.display_name, u.username) AS author_name,
               u.username AS author_username,
               up.bio AS author_bio,
               p.category_id, c.name AS category_name,
               p.status, p.published_at, p.created_at
        FROM posts p
        JOIN users u ON u.id = p.author_id
        LEFT JOIN user_profiles up ON up.user_id = p.author_id
        LEFT JOIN categories c ON c.id = p.category_id
        WHERE p.slug = ? AND p.status = 'published'
        "#,
    )
    .bind(&slug)
    .fetch_optional(&db.0)
    .await
    .map_err(sfe)?;
    Ok(post)
}
