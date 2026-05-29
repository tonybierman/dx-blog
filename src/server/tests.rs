//! Server-side unit tests: pure helpers (markdown render/sanitize), slug
//! uniqueness against a real (in-memory) SQLite pool, and the comment
//! auto-approve rule. Run with:
//!
//! ```text
//! cargo test --no-default-features --features server,sqlite
//! ```

use crate::server::{render_markdown, unique_slug};
use arium_dioxus::pool::Pool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;

/// A one-connection in-memory pool with the blog schema applied. `max_connections(1)`
/// keeps every query on the same `:memory:` database (separate connections would
/// each get their own empty one).
///
/// Foreign keys are turned OFF here: sqlx defaults them ON, but these tests apply
/// only the blog schema, not arium's `users` table that `posts.author_id` (and
/// friends) now reference — so a bare `author_id` would otherwise trip the FK.
async fn test_pool() -> Pool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("parse in-memory url")
        .foreign_keys(false);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("open in-memory sqlite");
    sqlx::raw_sql(include_str!("../../migrations/0001_blog.sql"))
        .execute(&pool)
        .await
        .expect("apply blog schema");
    pool
}

/// Insert a minimal post row (enough columns to satisfy NOT NULLs). Returns id.
async fn insert_post(pool: &Pool, title: &str, slug: &str) {
    // author_id references arium's users in production; FK enforcement is off in
    // test_pool (no users table here), so a bare id is fine for these unit tests.
    sqlx::query("INSERT INTO posts (title, slug, author_id) VALUES (?, ?, 1)")
        .bind(title)
        .bind(slug)
        .execute(pool)
        .await
        .expect("insert post");
}

// ---------------------------------------------------------------- markdown

#[test]
fn render_markdown_emits_html() {
    let html = render_markdown("# Title\n\nSome **bold** and a [link](https://example.com).");
    assert!(html.contains("<h1>"), "heading should render: {html}");
    assert!(html.contains("<strong>"), "bold should render: {html}");
    assert!(
        html.contains("href=\"https://example.com\""),
        "link should render: {html}"
    );
}

#[test]
fn render_markdown_strips_dangerous_html() {
    let html = render_markdown("Hello <script>alert('xss')</script> world");
    assert!(
        !html.contains("<script"),
        "script tag must be sanitized away: {html}"
    );
    assert!(
        html.contains("Hello"),
        "surrounding text should survive: {html}"
    );
}

#[test]
fn render_markdown_drops_onclick_attributes() {
    let html = render_markdown("<a href=\"#\" onclick=\"steal()\">x</a>");
    assert!(
        !html.contains("onclick"),
        "event handlers must be stripped: {html}"
    );
}

// ---------------------------------------------------------------- email sanity check

#[test]
fn looks_like_email_accepts_and_rejects() {
    use crate::server::looks_like_email;
    assert!(looks_like_email("a@b.com"));
    assert!(looks_like_email("first.last@sub.example.co"));
    // The junk the old `contains('@') && len >= 3` guard let through:
    assert!(!looks_like_email("a@b"), "needs a dotted domain");
    assert!(!looks_like_email("@x.com"), "empty local part");
    assert!(!looks_like_email("a@@b.com"), "double @");
    assert!(!looks_like_email("nope"), "no @");
    assert!(!looks_like_email("a@.com"), "empty domain label");
}

// ---------------------------------------------------------------- pagination + date helpers

#[test]
fn page_offset_clamps_and_computes() {
    use crate::model::{page_offset, PER_PAGE};
    assert_eq!(page_offset(0), (1, 0), "page < 1 clamps to 1");
    assert_eq!(page_offset(-5), (1, 0));
    assert_eq!(page_offset(1), (1, 0));
    assert_eq!(page_offset(3), (3, 2 * PER_PAGE));
}

#[test]
fn to_rfc3339_normalizes_sqlite_datetime() {
    use crate::model::to_rfc3339;
    assert_eq!(to_rfc3339("2024-01-02 03:04:05"), "2024-01-02T03:04:05Z");
    // Already ISO 8601 — passed through unchanged.
    assert_eq!(to_rfc3339("2024-01-02T03:04:05Z"), "2024-01-02T03:04:05Z");
    assert_eq!(to_rfc3339(""), "");
}

// ---------------------------------------------------------------- slug uniqueness

#[tokio::test]
async fn unique_slug_is_stable_when_free() {
    let pool = test_pool().await;
    let slug = unique_slug(&pool, "posts", "Hello World").await.unwrap();
    assert_eq!(slug, "hello-world");
}

#[tokio::test]
async fn unique_slug_appends_suffix_on_collision() {
    let pool = test_pool().await;
    insert_post(&pool, "Hello World", "hello-world").await;
    let slug = unique_slug(&pool, "posts", "Hello World").await.unwrap();
    assert_eq!(slug, "hello-world-2");

    // A second collision bumps to -3.
    insert_post(&pool, "Hello World 2", "hello-world-2").await;
    let slug = unique_slug(&pool, "posts", "Hello World").await.unwrap();
    assert_eq!(slug, "hello-world-3");
}

#[tokio::test]
async fn unique_slug_falls_back_for_empty_title() {
    let pool = test_pool().await;
    let slug = unique_slug(&pool, "posts", "!!!").await.unwrap();
    assert_eq!(slug, "item");
}

// ---------------------------------------------------------------- comment auto-approve

/// Mirrors the rule in `create_comment`: a logged-in author with a prior
/// *approved* comment is auto-approved; otherwise the comment stays pending.
async fn has_prior_approved(pool: &Pool, author_id: i64) -> bool {
    let prior: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM comments WHERE author_id = ? AND status = 'approved' LIMIT 1",
    )
    .bind(author_id)
    .fetch_optional(pool)
    .await
    .unwrap();
    prior.is_some()
}

#[tokio::test]
async fn returning_approved_commenter_is_recognized() {
    let pool = test_pool().await;
    // Author 1 already has an approved comment; author 2 only a pending one.
    sqlx::query(
        "INSERT INTO comments (post_id, author_id, body, status) VALUES (1, 1, 'hi', 'approved')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO comments (post_id, author_id, body, status) VALUES (1, 2, 'hi', 'pending')",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(
        has_prior_approved(&pool, 1).await,
        "author 1 should auto-approve"
    );
    assert!(
        !has_prior_approved(&pool, 2).await,
        "author 2 should stay pending"
    );
    assert!(
        !has_prior_approved(&pool, 99).await,
        "unknown author stays pending"
    );
}
