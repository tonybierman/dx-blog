//! Server-side unit tests: pure helpers (markdown render/sanitize), slug
//! uniqueness against a real (in-memory) SQLite pool, and the comment
//! auto-approve rule. Run with:
//!
//! ```text
//! cargo test --no-default-features --features server,sqlite
//! ```

use crate::db::unique_slug;
use crate::server::render_markdown;
use arium_dioxus::pool::Pool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;

/// A one-connection in-memory pool with the blog schema applied. `max_connections(1)`
/// keeps every query on the same `:memory:` database (separate connections would
/// each get their own empty one).
///
/// Foreign keys are turned ON to match production (`main.rs`), so the schema's
/// `REFERENCES … ON DELETE` clauses are actually exercised. The blog schema
/// references arium's `users` table, which these tests don't run arium's migrator
/// for, so we stand up a minimal `users(id)` parent first and seed the author ids
/// the fixtures attribute rows to.
async fn test_pool() -> Pool {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("parse in-memory url")
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("open in-memory sqlite");
    // Stand-in for arium's `users` table — only `id` is referenced by the blog
    // schema's FKs. Created before the blog schema so those REFERENCES resolve.
    sqlx::query("CREATE TABLE users (id INTEGER PRIMARY KEY)")
        .execute(&pool)
        .await
        .expect("create users stub");
    sqlx::raw_sql(include_str!("../../migrations/0001_blog.sql"))
        .execute(&pool)
        .await
        .expect("apply blog schema");
    // Author rows the fixtures attribute posts/comments to.
    for id in [1_i64, 2] {
        sqlx::query("INSERT INTO users (id) VALUES (?)")
            .bind(id)
            .execute(&pool)
            .await
            .expect("seed user");
    }
    pool
}

/// Insert a minimal post row (enough columns to satisfy NOT NULLs), attributed
/// to seeded user 1. Returns nothing; the first post inserted has id 1.
async fn insert_post(pool: &Pool, title: &str, slug: &str) {
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

#[test]
fn highlighted_code_emits_classed_spans() {
    use crate::server::highlight::render_markdown_highlighted;
    let html = render_markdown_highlighted("```rust\nfn main() {}\n```");
    // Syntect emitted prefixed token classes...
    assert!(
        html.contains("class=\"syn-"),
        "code should be highlighted with syn- classes: {html}"
    );
    // ...and ammonia kept the class attribute instead of sanitizing it away.
    assert!(html.contains("<pre"), "code block should survive: {html}");
    assert!(
        html.contains("class=\"language-rust\""),
        "language class should be present: {html}"
    );
    // The language label is driven by data-lang on <pre>.
    assert!(
        html.contains("data-lang=\"rust\""),
        "language label attribute should be present: {html}"
    );
}

#[test]
fn unlabeled_code_block_has_no_lang_attribute() {
    use crate::server::highlight::render_markdown_highlighted;
    // A fence with no language gets no label (and falls back to plain text).
    let html = render_markdown_highlighted("```\nplain text\n```");
    assert!(html.contains("<pre"), "code block should render: {html}");
    assert!(
        !html.contains("data-lang"),
        "no language → no label: {html}"
    );
}

#[test]
fn highlighted_prose_matches_plain_pipeline() {
    use crate::server::highlight::render_markdown_highlighted;
    // Non-code markdown must render identically to the shared plain pipeline.
    let md = "# Title\n\nSome **bold** text.";
    assert_eq!(render_markdown(md), render_markdown_highlighted(md));
}

// ---------------------------------------------------------------- mdx / embeds

use crate::mdx::{parse_body, Segment};

#[test]
fn parse_body_without_embeds_is_single_html_run() {
    let md = "# Title\n\nSome **bold** text.";
    let segs = parse_body(md);
    assert_eq!(segs.len(), 1, "no embeds → one run: {segs:?}");
    match &segs[0] {
        Segment::Html(html) => assert_eq!(
            html,
            &render_markdown(md),
            "prose run must match render_markdown byte-for-byte"
        ),
        other => panic!("expected Html, got {other:?}"),
    }
}

#[test]
fn parse_body_splits_prose_around_embed() {
    let segs = parse_body("before\n\n[[component:counter start=2]]\n\nafter");
    assert_eq!(segs.len(), 3, "prose / embed / prose: {segs:?}");
    assert!(matches!(segs[0], Segment::Html(_)));
    assert!(matches!(segs[2], Segment::Html(_)));
    match &segs[1] {
        Segment::Embed { name, props } => {
            assert_eq!(name, "counter");
            assert_eq!(props.get("start").map(String::as_str), Some("2"));
        }
        other => panic!("expected Embed, got {other:?}"),
    }
}

#[test]
fn parse_body_handles_bare_and_quoted_props() {
    let segs = parse_body("[[component:chart data=\"3, 7, 2\" kind=line wide]]");
    assert_eq!(segs.len(), 1);
    match &segs[0] {
        Segment::Embed { name, props } => {
            assert_eq!(name, "chart");
            assert_eq!(props.get("data").map(String::as_str), Some("3, 7, 2"));
            assert_eq!(props.get("kind").map(String::as_str), Some("line"));
            assert_eq!(props.get("wide").map(String::as_str), Some(""), "bare flag");
        }
        other => panic!("expected Embed, got {other:?}"),
    }
}

#[test]
fn parse_body_keeps_unknown_component_as_embed() {
    let segs = parse_body("[[component:nope x=1]]");
    assert!(
        matches!(&segs[0], Segment::Embed { name, .. } if name == "nope"),
        "unknown names still parse to an Embed (the registry renders a fallback): {segs:?}"
    );
}

#[test]
fn parse_body_ignores_non_embed_brackets() {
    // A normal markdown line that merely contains brackets is prose, not an embed.
    let segs = parse_body("See [[wiki style]] links and [refs](https://e.com).");
    assert_eq!(segs.len(), 1);
    assert!(matches!(segs[0], Segment::Html(_)));
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
    // A post for the comments to hang off (FKs are enforced now). First insert → id 1.
    insert_post(&pool, "Post", "post").await;
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

// ---------------------------------------------------------------- FK cascade

/// With foreign keys enabled, deleting a post should cascade to its dependent
/// rows (here: comments via `ON DELETE CASCADE`). This exercises the schema
/// constraint that `delete_post`'s hand-rolled deletes are only a fallback for.
#[tokio::test]
async fn deleting_post_cascades_to_comments() {
    let pool = test_pool().await;
    insert_post(&pool, "Post", "post").await; // id 1
    sqlx::query(
        "INSERT INTO comments (post_id, author_id, body, status) VALUES (1, 1, 'hi', 'approved')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM comments")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, 1, "comment should be inserted");

    sqlx::query("DELETE FROM posts WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();

    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM comments")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after, 0, "deleting the post should cascade to its comments");
}
