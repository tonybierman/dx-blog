//! Full-text search over posts via the `posts_fts` FTS5 table.

use dioxus::prelude::*;

use crate::model::PostFeed;
// Used only inside the server-fn body, which compiles out on the wasm client.
#[cfg(feature = "server")]
use crate::model::{page_offset, PER_PAGE};

#[cfg(feature = "server")]
use crate::server::{sfe, DbExtension, POST_CARD_COLUMNS, POST_CARD_JOINS};

#[post("/api/search", db: DbExtension)]
pub async fn search_posts(
    q: String,
    page: i64,
    category_slug: Option<String>,
    tag_slug: Option<String>,
    date_range: Option<String>,
) -> Result<PostFeed> {
    use crate::model::PostCard;

    let q = q.trim().to_string();
    if q.is_empty() {
        return Ok(PostFeed::empty());
    }
    let (page, offset) = page_offset(page);
    // Prefix-match each term so partial words hit; quote to neutralise FTS syntax.
    let fts_query = q
        .split_whitespace()
        .map(|t| format!("\"{}\"*", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ");

    // Normalise optional facets. Empty strings (from cleared <select>s) count as absent.
    let category_slug = category_slug.filter(|s| !s.is_empty());
    let tag_slug = tag_slug.filter(|s| !s.is_empty());
    // Map the date bucket to a SQLite datetime modifier (chosen from constants,
    // never interpolated from user input). Unknown / "any" → no date filter.
    let date_offset: Option<&'static str> = match date_range.as_deref() {
        Some("week") => Some("-7 days"),
        Some("month") => Some("-30 days"),
        Some("year") => Some("-365 days"),
        _ => None,
    };

    // Build the shared facet WHERE fragment. Placeholders bind in this order:
    // [category_slug?] [tag_slug?] [date_offset?].
    let mut facets = String::new();
    if category_slug.is_some() {
        facets.push_str(" AND p.category_id = (SELECT id FROM categories WHERE slug = ?)");
    }
    if tag_slug.is_some() {
        facets.push_str(
            " AND EXISTS (SELECT 1 FROM post_tags pt JOIN tags t ON t.id = pt.tag_id \
             WHERE pt.post_id = p.id AND t.slug = ?)",
        );
    }
    if date_offset.is_some() {
        facets.push_str(" AND p.published_at >= datetime('now', ?)");
    }

    let items_sql = format!(
        "SELECT {POST_CARD_COLUMNS} \
         FROM posts_fts f JOIN posts p ON p.id = f.rowid {POST_CARD_JOINS} \
         WHERE posts_fts MATCH ? AND p.status = 'published'{facets} \
         ORDER BY rank \
         LIMIT ? OFFSET ?"
    );
    let mut items_q = sqlx::query_as::<_, PostCard>(&items_sql).bind(&fts_query);
    if let Some(c) = &category_slug {
        items_q = items_q.bind(c);
    }
    if let Some(t) = &tag_slug {
        items_q = items_q.bind(t);
    }
    if let Some(off) = date_offset {
        items_q = items_q.bind(off);
    }
    let items = items_q
        .bind(PER_PAGE)
        .bind(offset)
        .fetch_all(&db.0)
        .await
        .map_err(sfe)?;

    let count_sql = format!(
        r#"
        SELECT COUNT(*)
        FROM posts_fts f
        JOIN posts p ON p.id = f.rowid
        WHERE posts_fts MATCH ? AND p.status = 'published'{facets}
        "#
    );
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql).bind(&fts_query);
    if let Some(c) = &category_slug {
        count_q = count_q.bind(c);
    }
    if let Some(t) = &tag_slug {
        count_q = count_q.bind(t);
    }
    if let Some(off) = date_offset {
        count_q = count_q.bind(off);
    }
    let total = count_q.fetch_one(&db.0).await.map_err(sfe)?;

    Ok(PostFeed::new(items, total, page))
}
