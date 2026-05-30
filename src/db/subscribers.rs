use sqlx::SqlitePool;

pub async fn upsert_subscriber_db(pool: &SqlitePool, email: &str) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT OR IGNORE INTO subscribers (email) VALUES (?)")
        .bind(email)
        .execute(pool)
        .await?;
    Ok(())
}

/// Returns `(id, confirmed)` for the subscriber with the given email.
pub async fn get_subscriber_by_email_db(
    pool: &SqlitePool,
    email: &str,
) -> Result<(i64, i64), sqlx::Error> {
    sqlx::query_as("SELECT id, confirmed FROM subscribers WHERE email = ?")
        .bind(email)
        .fetch_one(pool)
        .await
}

/// Returns `true` if a token was issued for `subscriber_id` within `cooldown`
/// (a SQLite datetime offset string like `"-5 minutes"`).
pub async fn has_recent_token_db(
    pool: &SqlitePool,
    subscriber_id: i64,
    cooldown: &str,
) -> Result<bool, sqlx::Error> {
    let row: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM subscriber_tokens \
         WHERE subscriber_id = ? AND created_at >= datetime('now', ?) LIMIT 1",
    )
    .bind(subscriber_id)
    .bind(cooldown)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

/// Atomically rotate the confirmation token for `subscriber_id`: delete any
/// existing token and insert a fresh one. Returns the new token string.
pub async fn rotate_subscriber_token_db(
    pool: &SqlitePool,
    subscriber_id: i64,
) -> Result<String, sqlx::Error> {
    let token: String = sqlx::query_scalar("SELECT lower(hex(randomblob(16)))")
        .fetch_one(pool)
        .await?;
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM subscriber_tokens WHERE subscriber_id = ?")
        .bind(subscriber_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO subscriber_tokens (token, subscriber_id) VALUES (?, ?)")
        .bind(&token)
        .bind(subscriber_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(token)
}

/// Consume a confirmation token: flip `confirmed = 1` and delete all tokens for
/// that subscriber in one transaction. Returns `true` if a valid, unexpired token
/// matched; `false` if not found or past the TTL.
pub async fn confirm_subscriber_db(
    pool: &SqlitePool,
    token: &str,
    token_ttl: &str,
) -> Result<bool, sqlx::Error> {
    let sub_id: Option<i64> = sqlx::query_scalar(
        "SELECT subscriber_id FROM subscriber_tokens \
         WHERE token = ? AND created_at >= datetime('now', ?)",
    )
    .bind(token)
    .bind(token_ttl)
    .fetch_optional(pool)
    .await?;

    let Some(sub_id) = sub_id else {
        return Ok(false);
    };

    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE subscribers SET confirmed = 1 WHERE id = ?")
        .bind(sub_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM subscriber_tokens WHERE subscriber_id = ?")
        .bind(sub_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}
