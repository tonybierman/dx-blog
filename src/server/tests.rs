//! Server-side unit tests: pure helpers (markdown render/sanitize), slug
//! uniqueness against a real (in-memory) SQLite pool, and the comment
//! auto-approve rule. Run with:
//!
//! ```text
//! cargo test --no-default-features --features server,sqlite
//! ```

use crate::server::{render_markdown, unique_slug};
use arium_dioxus::pool::Pool;
use sqlx::sqlite::SqlitePoolOptions;

/// A one-connection in-memory pool with the blog schema applied. `max_connections(1)`
/// keeps every query on the same `:memory:` database (separate connections would
/// each get their own empty one).
async fn test_pool() -> Pool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
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
    // author_id references arium's users in production, but there's no FK in the
    // blog schema, so a bare id is fine for these unit tests.
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
