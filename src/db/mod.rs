//! Database access layer. Each submodule owns all SQL for one domain.
//! Server functions call these functions, then map `sqlx::Error` to
//! `ServerFnError` with `sfe()` — that mapping never lives here.
//!
//! Pool type: `arium_dioxus::pool::Pool` is the compile-time backend alias
//! (`SqlitePool` or `PgPool` depending on the active feature), so db functions
//! accept `&Pool` and callers pass `&db.0` directly.

pub mod analytics;
pub mod authors;
pub mod comments;
pub mod dialect;
pub mod feeds;
pub mod media;
pub mod posts;
pub mod reactions;
pub mod settings;
pub mod subscribers;
pub mod taxonomy;

use arium_dioxus::pool::Pool;

/// The `PostCard` 10-column projection shared across every feed, search, and
/// analytics read path. Compose with `format!`; the `FROM` clause and bind
/// order live in each caller.
pub const POST_CARD_COLUMNS: &str = "p.id, p.title, p.slug, p.excerpt, p.featured_image_url, \
     p.author_id, COALESCE(u.display_name, u.username) AS author_name, \
     u.username AS author_username, \
     c.name AS category_name, p.status, p.published_at";

/// The joins `POST_CARD_COLUMNS` depends on. Place right after a `FROM` that
/// aliases the posts row as `p`.
pub const POST_CARD_JOINS: &str =
    "JOIN users u ON u.id = p.author_id LEFT JOIN categories c ON c.id = p.category_id";

/// Generate a unique slug for `name` within `table`, appending `-2`, `-3`, …
/// on collision. `table` is always an internal constant, never user input.
pub async fn unique_slug(pool: &Pool, table: &str, name: &str) -> Result<String, sqlx::Error> {
    let base = {
        let s = slug::slugify(name);
        if s.is_empty() {
            "item".to_string()
        } else {
            s
        }
    };
    let mut candidate = base.clone();
    let mut n = 2;
    let sql = format!("SELECT id FROM {table} WHERE slug = $1");
    loop {
        let exists: Option<i64> = sqlx::query_scalar(&sql)
            .bind(&candidate)
            .fetch_optional(pool)
            .await?;
        if exists.is_none() {
            return Ok(candidate);
        }
        candidate = format!("{base}-{n}");
        n += 1;
    }
}
