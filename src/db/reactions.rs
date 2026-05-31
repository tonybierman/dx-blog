use crate::db::dialect;
use arium_dioxus::pool::Pool;

pub async fn post_is_published_db(pool: &Pool, post_id: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM posts WHERE id = $1 AND status = 'published')")
        .bind(post_id)
        .fetch_one(pool)
        .await
}

pub async fn reaction_visitor_total_db(
    pool: &Pool,
    post_id: i64,
    visitor_hash: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM reactions WHERE post_id = $1 AND visitor_hash = $2")
        .bind(post_id)
        .bind(visitor_hash)
        .fetch_one(pool)
        .await
}

pub async fn reaction_burst_count_db(
    pool: &Pool,
    post_id: i64,
    visitor_hash: &str,
    burst_window: &str,
) -> Result<i64, sqlx::Error> {
    let cutoff = dialect::now_offset(3);
    sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM reactions \
         WHERE post_id = $1 AND visitor_hash = $2 AND created_at >= {cutoff}",
    ))
    .bind(post_id)
    .bind(visitor_hash)
    .bind(burst_window)
    .fetch_one(pool)
    .await
}

pub async fn insert_reaction_db(
    pool: &Pool,
    post_id: i64,
    kind: &str,
    visitor_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO reactions (post_id, kind, visitor_hash) VALUES ($1, $2, $3)")
        .bind(post_id)
        .bind(kind)
        .bind(visitor_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn reaction_total_db(pool: &Pool, post_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM reactions WHERE post_id = $1")
        .bind(post_id)
        .fetch_one(pool)
        .await
}

/// A post's `(title, slug)`, for labelling the admin live activity feed. One
/// indexed PK lookup; called on the (rate-limited) reaction path.
pub async fn post_title_slug_db(
    pool: &Pool,
    post_id: i64,
) -> Result<(String, String), sqlx::Error> {
    sqlx::query_as::<_, (String, String)>("SELECT title, slug FROM posts WHERE id = $1")
        .bind(post_id)
        .fetch_one(pool)
        .await
}
