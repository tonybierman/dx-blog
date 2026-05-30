use crate::db::{POST_CARD_COLUMNS, POST_CARD_JOINS};
use crate::model::{PostCard, PostDetail, PostEditData};
use sqlx::SqlitePool;

/// Shared WHERE clause for the published feed + its COUNT companion.
/// Bind order: `(category_slug, category_slug, tag_slug, tag_slug)`.
const LIST_POSTS_WHERE: &str = "WHERE p.status = 'published' \
     AND (? IS NULL OR c.slug = ?) \
     AND (? IS NULL OR EXISTS ( \
           SELECT 1 FROM post_tags pt JOIN tags t ON t.id = pt.tag_id \
           WHERE pt.post_id = p.id AND t.slug = ?))";

/// Returns `(items, total_count)` for the published feed.
pub async fn list_posts_db(
    pool: &SqlitePool,
    limit: i64,
    offset: i64,
    category_slug: Option<&str>,
    tag_slug: Option<&str>,
) -> Result<(Vec<PostCard>, i64), sqlx::Error> {
    let items = sqlx::query_as::<_, PostCard>(&format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} {LIST_POSTS_WHERE} \
         ORDER BY p.published_at DESC, p.id DESC \
         LIMIT ? OFFSET ?"
    ))
    .bind(category_slug)
    .bind(category_slug)
    .bind(tag_slug)
    .bind(tag_slug)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM posts p {POST_CARD_JOINS} {LIST_POSTS_WHERE}"
    ))
    .bind(category_slug)
    .bind(category_slug)
    .bind(tag_slug)
    .bind(tag_slug)
    .fetch_one(pool)
    .await?;

    Ok((items, total))
}

pub async fn list_archive_db(
    pool: &SqlitePool,
    limit: i64,
    offset: i64,
) -> Result<(Vec<PostCard>, i64), sqlx::Error> {
    let items = sqlx::query_as::<_, PostCard>(&format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         WHERE p.status = 'published' \
         ORDER BY p.published_at DESC, p.id DESC \
         LIMIT ? OFFSET ?"
    ))
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM posts WHERE status = 'published'")
        .fetch_one(pool)
        .await?;

    Ok((items, total))
}

pub async fn posts_by_author_db(
    pool: &SqlitePool,
    username: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<PostCard>, i64), sqlx::Error> {
    let items = sqlx::query_as::<_, PostCard>(&format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         WHERE p.status = 'published' AND u.username = ? \
         ORDER BY p.published_at DESC, p.id DESC \
         LIMIT ? OFFSET ?"
    ))
    .bind(username)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM posts p JOIN users u ON u.id = p.author_id
        WHERE p.status = 'published' AND u.username = ?
        "#,
    )
    .bind(username)
    .fetch_one(pool)
    .await?;

    Ok((items, total))
}

pub async fn featured_posts_db(
    pool: &SqlitePool,
    limit: i64,
) -> Result<Vec<PostCard>, sqlx::Error> {
    sqlx::query_as::<_, PostCard>(&format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         LEFT JOIN (SELECT post_id, COUNT(*) AS views FROM post_views GROUP BY post_id) v \
           ON v.post_id = p.id \
         WHERE p.status = 'published' \
         ORDER BY COALESCE(v.views, 0) DESC, p.published_at DESC, p.id DESC \
         LIMIT ?"
    ))
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn get_post_db(pool: &SqlitePool, slug: &str) -> Result<Option<PostDetail>, sqlx::Error> {
    sqlx::query_as::<_, PostDetail>(
        r#"
        SELECT p.id, p.title, p.slug, p.body_md, p.body_html, p.excerpt,
               p.featured_image_url, p.author_id,
               COALESCE(u.display_name, u.username) AS author_name,
               u.username AS author_username,
               up.bio AS author_bio,
               p.category_id, c.name AS category_name,
               p.status, p.published_at, p.created_at
        FROM posts p
        JOIN users u ON u.id = p.author_id
        LEFT JOIN user_profiles up ON up.user_id = p.author_id
        LEFT JOIN categories c ON c.id = p.category_id
        WHERE p.slug = ?
        "#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
}

/// Returns the full edit-form data for a post (two queries: fields + tag ids).
pub async fn get_post_edit_db(pool: &SqlitePool, id: i64) -> Result<PostEditData, sqlx::Error> {
    let row: (
        String,
        String,
        String,
        String,
        Option<i64>,
        Option<String>,
        String,
    ) = sqlx::query_as(
        "SELECT title, slug, body_md, excerpt, category_id, featured_image_url, status
             FROM posts WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    let tag_ids: Vec<i64> = sqlx::query_scalar("SELECT tag_id FROM post_tags WHERE post_id = ?")
        .bind(id)
        .fetch_all(pool)
        .await?;

    Ok(PostEditData {
        id,
        title: row.0,
        slug: row.1,
        body_md: row.2,
        excerpt: row.3,
        category_id: row.4,
        featured_image_url: row.5,
        status: row.6,
        tag_ids,
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_post_db(
    pool: &SqlitePool,
    title: &str,
    slug: &str,
    body_md: &str,
    body_html: &str,
    excerpt: &str,
    author_id: i64,
    category_id: Option<i64>,
    featured_image_url: Option<&str>,
    status: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO posts
          (title, slug, body_md, body_html, excerpt, author_id, category_id,
           featured_image_url, status, published_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?,
           CASE WHEN ? = 'published' THEN datetime('now') ELSE NULL END)
        RETURNING id
        "#,
    )
    .bind(title)
    .bind(slug)
    .bind(body_md)
    .bind(body_html)
    .bind(excerpt)
    .bind(author_id)
    .bind(category_id)
    .bind(featured_image_url)
    .bind(status)
    .bind(status)
    .fetch_one(pool)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn update_post_db(
    pool: &SqlitePool,
    id: i64,
    title: &str,
    body_md: &str,
    body_html: &str,
    excerpt: &str,
    category_id: Option<i64>,
    featured_image_url: Option<&str>,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE posts SET
          title = ?, body_md = ?, body_html = ?, excerpt = ?,
          category_id = ?, featured_image_url = ?, status = ?,
          published_at = CASE WHEN ? = 'published' AND published_at IS NULL
                              THEN datetime('now') ELSE published_at END,
          updated_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(title)
    .bind(body_md)
    .bind(body_html)
    .bind(excerpt)
    .bind(category_id)
    .bind(featured_image_url)
    .bind(status)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Delete a post and all its child rows in a single transaction.
pub async fn delete_post_db(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for sql in [
        "DELETE FROM post_tags WHERE post_id = ?",
        "DELETE FROM comments WHERE post_id = ?",
        "DELETE FROM post_views WHERE post_id = ?",
        "DELETE FROM arium_resource_members WHERE kind = 'post' AND resource_id = ?",
        "DELETE FROM posts WHERE id = ?",
    ] {
        sqlx::query(sql).bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Replace a post's tags with `tag_ids` (delete-then-insert).
pub async fn set_post_tags_db(
    pool: &SqlitePool,
    post_id: i64,
    tag_ids: &[i64],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM post_tags WHERE post_id = ?")
        .bind(post_id)
        .execute(pool)
        .await?;
    for tid in tag_ids {
        sqlx::query("INSERT OR IGNORE INTO post_tags (post_id, tag_id) VALUES (?, ?)")
            .bind(post_id)
            .bind(tid)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// Admin list: author-scoped or global, with optional status filter and sort.
/// The ORDER BY clause is derived from whitelisted `sort` values (never interpolated from user input).
pub async fn admin_list_posts_db(
    pool: &SqlitePool,
    is_admin: bool,
    author_id: i64,
    status_filter: Option<&str>,
    sort: Option<&str>,
) -> Result<Vec<PostCard>, sqlx::Error> {
    let order_by = match sort {
        Some("title") => "p.title COLLATE NOCASE ASC, p.id DESC",
        Some("title_desc") => "p.title COLLATE NOCASE DESC, p.id DESC",
        Some("status") => "p.status ASC, p.updated_at DESC",
        Some("status_desc") => "p.status DESC, p.updated_at DESC",
        Some("published") => "p.published_at IS NULL, p.published_at DESC, p.id DESC",
        Some("published_desc") => "p.published_at IS NULL, p.published_at ASC, p.id ASC",
        Some("oldest") => "p.updated_at ASC, p.id ASC",
        _ => "p.updated_at DESC, p.id DESC",
    };
    let sql = format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         WHERE (? = 1 OR p.author_id = ?) AND (? IS NULL OR p.status = ?) \
         ORDER BY {order_by}"
    );
    sqlx::query_as::<_, PostCard>(&sql)
        .bind(is_admin as i64)
        .bind(author_id)
        .bind(status_filter)
        .bind(status_filter)
        .fetch_all(pool)
        .await
}

/// Full-text search over the `posts_fts` FTS5 table with optional facet filters.
/// `fts_query` must be pre-sanitised (caller splits on whitespace and quotes terms).
/// `date_offset` is a SQLite datetime modifier chosen from a server-side whitelist.
pub async fn search_posts_db(
    pool: &SqlitePool,
    fts_query: &str,
    limit: i64,
    offset: i64,
    category_slug: Option<&str>,
    tag_slug: Option<&str>,
    date_offset: Option<&str>,
) -> Result<(Vec<PostCard>, i64), sqlx::Error> {
    let mut facets = String::new();
    if category_slug.is_some() {
        facets.push_str(" AND p.category_id = (SELECT id FROM categories WHERE slug = ?)");
    }
    if tag_slug.is_some() {
        facets.push_str(
            " AND EXISTS (SELECT 1 FROM post_tags pt JOIN tags t ON t.id = pt.tag_id \
             WHERE pt.post_id = p.id AND t.slug = ?)",
        );
    }
    if date_offset.is_some() {
        facets.push_str(" AND p.published_at >= datetime('now', ?)");
    }

    let items_sql = format!(
        "SELECT {POST_CARD_COLUMNS} \
         FROM posts_fts f JOIN posts p ON p.id = f.rowid {POST_CARD_JOINS} \
         WHERE posts_fts MATCH ? AND p.status = 'published'{facets} \
         ORDER BY rank \
         LIMIT ? OFFSET ?"
    );
    let mut items_q = sqlx::query_as::<_, PostCard>(&items_sql).bind(fts_query);
    if let Some(c) = category_slug {
        items_q = items_q.bind(c);
    }
    if let Some(t) = tag_slug {
        items_q = items_q.bind(t);
    }
    if let Some(off) = date_offset {
        items_q = items_q.bind(off);
    }
    let items = items_q.bind(limit).bind(offset).fetch_all(pool).await?;

    let count_sql = format!(
        "SELECT COUNT(*) \
         FROM posts_fts f JOIN posts p ON p.id = f.rowid \
         WHERE posts_fts MATCH ? AND p.status = 'published'{facets}"
    );
    let mut count_q = sqlx::query_scalar::<_, i64>(&count_sql).bind(fts_query);
    if let Some(c) = category_slug {
        count_q = count_q.bind(c);
    }
    if let Some(t) = tag_slug {
        count_q = count_q.bind(t);
    }
    if let Some(off) = date_offset {
        count_q = count_q.bind(off);
    }
    let total = count_q.fetch_one(pool).await?;

    Ok((items, total))
}
