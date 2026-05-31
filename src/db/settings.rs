use arium_dioxus::pool::Pool;

pub async fn get_setting_db(pool: &Pool, key: &str) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT value FROM site_settings WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
}

pub async fn set_setting_db(pool: &Pool, key: &str, value: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO site_settings (key, value) VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}
