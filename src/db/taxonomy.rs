use crate::model::{Category, Tag};
use arium_dioxus::pool::Pool;

pub async fn list_categories_db(pool: &Pool) -> Result<Vec<Category>, sqlx::Error> {
    sqlx::query_as::<_, Category>(
        "SELECT id, name, slug, description FROM categories ORDER BY name",
    )
    .fetch_all(pool)
    .await
}

pub async fn list_tags_db(pool: &Pool) -> Result<Vec<Tag>, sqlx::Error> {
    sqlx::query_as::<_, Tag>("SELECT id, name, slug FROM tags ORDER BY name")
        .fetch_all(pool)
        .await
}

pub async fn get_category_db(pool: &Pool, slug: &str) -> Result<Option<Category>, sqlx::Error> {
    sqlx::query_as::<_, Category>(
        "SELECT id, name, slug, description FROM categories WHERE slug = $1",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
}

pub async fn get_tag_db(pool: &Pool, slug: &str) -> Result<Option<Tag>, sqlx::Error> {
    sqlx::query_as::<_, Tag>("SELECT id, name, slug FROM tags WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await
}

pub async fn insert_category_db(
    pool: &Pool,
    name: &str,
    slug: &str,
    description: Option<&str>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO categories (name, slug, description) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(name)
    .bind(slug)
    .bind(description)
    .fetch_one(pool)
    .await
}

pub async fn delete_category_db(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE posts SET category_id = NULL WHERE category_id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM categories WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn rename_category_db(pool: &Pool, id: i64, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE categories SET name = $1 WHERE id = $2")
        .bind(name)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn insert_tag_db(pool: &Pool, name: &str, slug: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>("INSERT INTO tags (name, slug) VALUES ($1, $2) RETURNING id")
        .bind(name)
        .bind(slug)
        .fetch_one(pool)
        .await
}

pub async fn delete_tag_db(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM post_tags WHERE tag_id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM tags WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn rename_tag_db(pool: &Pool, id: i64, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE tags SET name = $1 WHERE id = $2")
        .bind(name)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
