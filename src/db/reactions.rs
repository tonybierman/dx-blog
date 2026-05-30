use sqlx::SqlitePool;

pub async fn post_is_published_db(pool: &SqlitePool, post_id: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM posts WHERE id = ? AND status = 'published')")
        .bind(post_id)
        .fetch_one(pool)
        .await
}

pub async fn reaction_visitor_total_db(
    pool: &SqlitePool,
    post_id: i64,
    visitor_hash: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM reactions WHERE post_id = ? AND visitor_hash = ?")
        .bind(post_id)
        .bind(visitor_hash)
        .fetch_one(pool)
        .await
}

pub async fn reaction_burst_count_db(
    pool: &SqlitePool,
    post_id: i64,
    visitor_hash: &str,
    burst_window: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM reactions \
         WHERE post_id = ? AND visitor_hash = ? AND created_at >= datetime('now', ?)",
    )
    .bind(post_id)
    .bind(visitor_hash)
    .bind(burst_window)
    .fetch_one(pool)
    .await
}

pub async fn insert_reaction_db(
    pool: &SqlitePool,
    post_id: i64,
    kind: &str,
    visitor_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO reactions (post_id, kind, visitor_hash) VALUES (?, ?, ?)")
        .bind(post_id)
        .bind(kind)
        .bind(visitor_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn reaction_total_db(pool: &SqlitePool, post_id: i64) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM reactions WHERE post_id = ?")
        .bind(post_id)
        .fetch_one(pool)
        .await
}
