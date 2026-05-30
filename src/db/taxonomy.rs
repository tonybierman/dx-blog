use crate::model::{Category, Tag};
use sqlx::SqlitePool;

pub async fn list_categories_db(pool: &SqlitePool) -> Result<Vec<Category>, sqlx::Error> {
    sqlx::query_as::<_, Category>(
        "SELECT id, name, slug, description FROM categories ORDER BY name",
    )
    .fetch_all(pool)
    .await
}

pub async fn list_tags_db(pool: &SqlitePool) -> Result<Vec<Tag>, sqlx::Error> {
    sqlx::query_as::<_, Tag>("SELECT id, name, slug FROM tags ORDER BY name")
        .fetch_all(pool)
        .await
}

pub async fn get_category_db(
    pool: &SqlitePool,
    slug: &str,
) -> Result<Option<Category>, sqlx::Error> {
    sqlx::query_as::<_, Category>(
        "SELECT id, name, slug, description FROM categories WHERE slug = ?",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
}

pub async fn get_tag_db(pool: &SqlitePool, slug: &str) -> Result<Option<Tag>, sqlx::Error> {
    sqlx::query_as::<_, Tag>("SELECT id, name, slug FROM tags WHERE slug = ?")
        .bind(slug)
        .fetch_optional(pool)
        .await
}

pub async fn insert_category_db(
    pool: &SqlitePool,
    name: &str,
    slug: &str,
    description: Option<&str>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO categories (name, slug, description) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(name)
    .bind(slug)
    .bind(description)
    .fetch_one(pool)
    .await
}

pub async fn delete_category_db(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE posts SET category_id = NULL WHERE category_id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM categories WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn rename_category_db(pool: &SqlitePool, id: i64, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE categories SET name = ? WHERE id = ?")
        .bind(name)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn insert_tag_db(pool: &SqlitePool, name: &str, slug: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>("INSERT INTO tags (name, slug) VALUES (?, ?) RETURNING id")
        .bind(name)
        .bind(slug)
        .fetch_one(pool)
        .await
}

pub async fn delete_tag_db(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM post_tags WHERE tag_id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM tags WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn rename_tag_db(pool: &SqlitePool, id: i64, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE tags SET name = ? WHERE id = ?")
        .bind(name)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
