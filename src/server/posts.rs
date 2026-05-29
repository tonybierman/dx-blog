//! Post read endpoints (public). Authoring/mutation lives in `server::admin`.

use dioxus::prelude::*;

use crate::model::{page_offset, PostDetail, PostFeed, PER_PAGE};

#[cfg(feature = "server")]
use crate::server::{sfe, DbExtension, POST_CARD_COLUMNS, POST_CARD_JOINS};

/// Paginated published-post feed, optionally filtered by category or tag slug.
#[post("/api/posts", db: DbExtension)]
pub async fn list_posts(
    page: i64,
    category_slug: Option<String>,
    tag_slug: Option<String>,
) -> Result<PostFeed> {
    use crate::model::PostCard;
    let pool = &db.0;
    let (page, offset) = page_offset(page);

    let items = sqlx::query_as::<_, PostCard>(&format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         WHERE p.status = 'published' \
           AND (? IS NULL OR c.slug = ?) \
           AND (? IS NULL OR EXISTS ( \
                 SELECT 1 FROM post_tags pt JOIN tags t ON t.id = pt.tag_id \
                 WHERE pt.post_id = p.id AND t.slug = ?)) \
         ORDER BY p.published_at DESC, p.id DESC \
         LIMIT ? OFFSET ?"
    ))
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

    Ok(PostFeed::new(items, total, page))
}

/// All published posts, newest first — backs the Masonry archive. `#[post]`
/// (not `#[get]`) so the page argument can ride in the request body.
#[post("/api/archive", db: DbExtension)]
pub async fn list_archive(page: i64) -> Result<PostFeed> {
    use crate::model::PostCard;
    let pool = &db.0;
    let (page, offset) = page_offset(page);

    let items = sqlx::query_as::<_, PostCard>(&format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         WHERE p.status = 'published' \
         ORDER BY p.published_at DESC, p.id DESC \
         LIMIT ? OFFSET ?"
    ))
    .bind(PER_PAGE)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(sfe)?;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM posts WHERE status = 'published'")
        .fetch_one(pool)
        .await
        .map_err(sfe)?;

    Ok(PostFeed::new(items, total, page))
}

/// Published posts authored by a given username, paginated.
#[post("/api/author-posts", db: DbExtension)]
pub async fn posts_by_author(username: String, page: i64) -> Result<PostFeed> {
    use crate::model::PostCard;
    let pool = &db.0;
    let (page, offset) = page_offset(page);

    let items = sqlx::query_as::<_, PostCard>(&format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         WHERE p.status = 'published' AND u.username = ? \
         ORDER BY p.published_at DESC, p.id DESC \
         LIMIT ? OFFSET ?"
    ))
    .bind(&username)
    .bind(PER_PAGE)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(sfe)?;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM posts p JOIN users u ON u.id = p.author_id
        WHERE p.status = 'published' AND u.username = ?
        "#,
    )
    .bind(&username)
    .fetch_one(pool)
    .await
    .map_err(sfe)?;

    Ok(PostFeed::new(items, total, page))
}

/// The most-viewed published posts — backs the home "Featured" sidebar. Public;
/// falls back to the newest posts when there are no recorded views yet.
#[post("/api/posts/featured", db: DbExtension)]
pub async fn featured_posts(limit: i64) -> Result<Vec<crate::model::PostCard>> {
    use crate::model::PostCard;
    let limit = limit.clamp(1, 10);
    let rows = sqlx::query_as::<_, PostCard>(&format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         LEFT JOIN (SELECT post_id, COUNT(*) AS views FROM post_views GROUP BY post_id) v \
           ON v.post_id = p.id \
         WHERE p.status = 'published' \
         ORDER BY COALESCE(v.views, 0) DESC, p.published_at DESC, p.id DESC \
         LIMIT ?"
    ))
    .bind(limit)
    .fetch_all(&db.0)
    .await
    .map_err(sfe)?;
    Ok(rows)
}

/// A single post by slug, with author + category joined in. Published posts are
/// public; an unpublished (draft) post is returned only to a caller who can edit
/// it (Editor+ on the post, or a global admin) so authors can preview their own
/// drafts. Everyone else gets `None` for a draft.
#[post("/api/post", auth: arium_dioxus::auth::Session, db: DbExtension, authority: arium_dioxus::ResourceAuthorityExt)]
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
        WHERE p.slug = ?
        "#,
    )
    .bind(&slug)
    .fetch_optional(&db.0)
    .await
    .map_err(sfe)?;

    // Hide drafts from anyone who can't edit them.
    if let Some(ref p) = post {
        if p.status != "published"
            && crate::server::admin::can_edit_post(&auth, &db.0, &authority, p.id)
                .await
                .is_err()
        {
            return Ok(None);
        }
    }
    Ok(post)
}
