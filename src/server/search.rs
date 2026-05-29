//! Full-text search over posts via the `posts_fts` FTS5 table.

use dioxus::prelude::*;

use crate::model::{PostFeed, PER_PAGE};

#[cfg(feature = "server")]
use crate::server::{sfe, DbExtension};

#[post("/api/search", db: DbExtension)]
pub async fn search_posts(q: String, page: i64) -> Result<PostFeed> {
    use crate::model::PostCard;

    let q = q.trim().to_string();
    if q.is_empty() {
        return Ok(PostFeed {
            items: vec![],
            total: 0,
            page: 1,
            per_page: PER_PAGE,
        });
    }
    let page = page.max(1);
    let offset = (page - 1) * PER_PAGE;
    // Prefix-match each term so partial words hit; quote to neutralise FTS syntax.
    let fts_query = q
        .split_whitespace()
        .map(|t| format!("\"{}\"*", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ");

    let items = sqlx::query_as::<_, PostCard>(
        r#"
        SELECT p.id, p.title, p.slug, p.excerpt, p.featured_image_url,
               p.author_id,
               COALESCE(u.display_name, u.username) AS author_name,
               c.name AS category_name,
               p.status, p.published_at
        FROM posts_fts f
        JOIN posts p ON p.id = f.rowid
        JOIN users u ON u.id = p.author_id
        LEFT JOIN categories c ON c.id = p.category_id
        WHERE posts_fts MATCH ? AND p.status = 'published'
        ORDER BY rank
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(&fts_query)
    .bind(PER_PAGE)
    .bind(offset)
    .fetch_all(&db.0)
    .await
    .map_err(sfe)?;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM posts_fts f
        JOIN posts p ON p.id = f.rowid
        WHERE posts_fts MATCH ? AND p.status = 'published'
        "#,
    )
    .bind(&fts_query)
    .fetch_one(&db.0)
    .await
    .map_err(sfe)?;

    Ok(PostFeed {
        items,
        total,
        page,
        per_page: PER_PAGE,
    })
}
