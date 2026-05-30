use crate::model::MediaItem;
use sqlx::SqlitePool;

pub async fn list_media_db(pool: &SqlitePool) -> Result<Vec<MediaItem>, sqlx::Error> {
    sqlx::query_as::<_, MediaItem>(
        "SELECT id, filename, url, uploaded_by, created_at FROM media ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn insert_media_stub_db(
    pool: &SqlitePool,
    filename: &str,
    uploaded_by: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO media (filename, url, uploaded_by) VALUES (?, '', ?) RETURNING id",
    )
    .bind(filename)
    .bind(uploaded_by)
    .fetch_one(pool)
    .await
}

pub async fn update_media_url_db(pool: &SqlitePool, id: i64, url: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE media SET url = ? WHERE id = ?")
        .bind(url)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_media_created_at_db(pool: &SqlitePool, id: i64) -> Result<String, sqlx::Error> {
    sqlx::query_scalar("SELECT created_at FROM media WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn get_media_row_db(
    pool: &SqlitePool,
    id: i64,
) -> Result<Option<(String, i64)>, sqlx::Error> {
    sqlx::query_as("SELECT url, uploaded_by FROM media WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn delete_media_db(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM media WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
