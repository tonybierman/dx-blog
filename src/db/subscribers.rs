use crate::db::dialect::{self, RANDOM_HEX_16};
use arium_dioxus::pool::Pool;

pub async fn upsert_subscriber_db(pool: &Pool, email: &str) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO subscribers (email) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(email)
        .execute(pool)
        .await?;
    Ok(())
}

/// Returns `(id, confirmed)` for the subscriber with the given email.
pub async fn get_subscriber_by_email_db(
    pool: &Pool,
    email: &str,
) -> Result<(i64, i64), sqlx::Error> {
    sqlx::query_as("SELECT id, confirmed FROM subscribers WHERE email = $1")
        .bind(email)
        .fetch_one(pool)
        .await
}

/// Returns `true` if a token was issued for `subscriber_id` within `cooldown`
/// (a SQLite datetime offset string like `"-5 minutes"`).
pub async fn has_recent_token_db(
    pool: &Pool,
    subscriber_id: i64,
    cooldown: &str,
) -> Result<bool, sqlx::Error> {
    let cutoff = dialect::now_offset(2);
    let row: Option<i64> = sqlx::query_scalar(&format!(
        "SELECT 1 FROM subscriber_tokens \
         WHERE subscriber_id = $1 AND created_at >= {cutoff} LIMIT 1",
    ))
    .bind(subscriber_id)
    .bind(cooldown)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

/// Atomically rotate the confirmation token for `subscriber_id`: delete any
/// existing token and insert a fresh one. Returns the new token string.
pub async fn rotate_subscriber_token_db(
    pool: &Pool,
    subscriber_id: i64,
) -> Result<String, sqlx::Error> {
    let token: String = sqlx::query_scalar(&format!("SELECT {RANDOM_HEX_16}"))
        .fetch_one(pool)
        .await?;
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM subscriber_tokens WHERE subscriber_id = $1")
        .bind(subscriber_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO subscriber_tokens (token, subscriber_id) VALUES ($1, $2)")
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
    pool: &Pool,
    token: &str,
    token_ttl: &str,
) -> Result<bool, sqlx::Error> {
    let cutoff = dialect::now_offset(2);
    let sub_id: Option<i64> = sqlx::query_scalar(&format!(
        "SELECT subscriber_id FROM subscriber_tokens \
         WHERE token = $1 AND created_at >= {cutoff}",
    ))
    .bind(token)
    .bind(token_ttl)
    .fetch_optional(pool)
    .await?;

    let Some(sub_id) = sub_id else {
        return Ok(false);
    };

    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE subscribers SET confirmed = TRUE WHERE id = $1")
        .bind(sub_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM subscriber_tokens WHERE subscriber_id = $1")
        .bind(sub_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}
