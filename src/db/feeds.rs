use arium_dioxus::pool::Pool;
use chrono::{DateTime, Utc};

/// A row returned by the atom feed query.
#[derive(sqlx::FromRow)]
pub struct FeedRow {
    pub title: String,
    pub slug: String,
    pub excerpt: String,
    pub body_html: String,
    pub published_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub author_name: String,
}

/// Returns `(slug, lastmod)` for every published post, newest first. `lastmod`
/// falls back through the timestamp columns, so it's nullable in principle
/// (all three are NOT NULL by schema, so in practice always `Some`).
pub async fn feed_published_posts_db(
    pool: &Pool,
) -> Result<Vec<(String, Option<DateTime<Utc>>)>, sqlx::Error> {
    sqlx::query_as::<_, (String, Option<DateTime<Utc>>)>(
        r#"
        SELECT slug, COALESCE(updated_at, published_at, created_at) AS lastmod
        FROM posts
        WHERE status = 'published'
        ORDER BY lastmod DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn feed_category_slugs_db(pool: &Pool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT slug FROM categories ORDER BY slug")
        .fetch_all(pool)
        .await
}

pub async fn feed_tag_slugs_db(pool: &Pool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT slug FROM tags ORDER BY slug")
        .fetch_all(pool)
        .await
}

pub async fn feed_active_author_usernames_db(pool: &Pool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT DISTINCT u.username
        FROM users u
        JOIN posts p ON p.author_id = u.id
        WHERE p.status = 'published'
        ORDER BY u.username
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn feed_atom_posts_db(pool: &Pool, limit: i64) -> Result<Vec<FeedRow>, sqlx::Error> {
    sqlx::query_as::<_, FeedRow>(
        r#"
        SELECT p.title, p.slug, p.excerpt, p.body_html,
               p.published_at, p.updated_at,
               COALESCE(u.display_name, u.username) AS author_name
        FROM posts p
        JOIN users u ON u.id = p.author_id
        WHERE p.status = 'published'
        ORDER BY p.published_at DESC, p.id DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}
