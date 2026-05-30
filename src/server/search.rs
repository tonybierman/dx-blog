//! Full-text search over posts via the `posts_fts` FTS5 table.

use dioxus::prelude::*;

use crate::model::PostFeed;
#[cfg(feature = "server")]
use crate::model::{page_offset, PER_PAGE};

#[cfg(feature = "server")]
use crate::db::posts::search_posts_db;
#[cfg(feature = "server")]
use crate::server::{sfe, DbExtension};

#[post("/api/search", db: DbExtension)]
pub async fn search_posts(
    q: String,
    page: i64,
    category_slug: Option<String>,
    tag_slug: Option<String>,
    date_range: Option<String>,
) -> Result<PostFeed> {
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

    let (items, total) = search_posts_db(
        &db.0,
        &fts_query,
        PER_PAGE,
        offset,
        category_slug.as_deref(),
        tag_slug.as_deref(),
        date_offset,
    )
    .await
    .map_err(sfe)?;

    Ok(PostFeed::new(items, total, page))
}
