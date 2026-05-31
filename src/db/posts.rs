use crate::db::dialect::{self, NOW};
use crate::db::{POST_CARD_COLUMNS, POST_CARD_JOINS};
use crate::model::{PostCard, PostDetail, PostEditData};
use arium_dioxus::pool::Pool;

/// Shared WHERE clause for the published feed + its COUNT companion.
/// Bind order: `(category_slug, category_slug, tag_slug, tag_slug)`.
const LIST_POSTS_WHERE: &str = "WHERE p.status = 'published' \
     AND ($1 IS NULL OR c.slug = $2) \
     AND ($3 IS NULL OR EXISTS ( \
           SELECT 1 FROM post_tags pt JOIN tags t ON t.id = pt.tag_id \
           WHERE pt.post_id = p.id AND t.slug = $4))";

/// Returns `(items, total_count)` for the published feed.
pub async fn list_posts_db(
    pool: &Pool,
    limit: i64,
    offset: i64,
    category_slug: Option<&str>,
    tag_slug: Option<&str>,
) -> Result<(Vec<PostCard>, i64), sqlx::Error> {
    let items = sqlx::query_as::<_, PostCard>(&format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} {LIST_POSTS_WHERE} \
         ORDER BY p.published_at DESC, p.id DESC \
         LIMIT $5 OFFSET $6"
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
    pool: &Pool,
    limit: i64,
    offset: i64,
) -> Result<(Vec<PostCard>, i64), sqlx::Error> {
    let items = sqlx::query_as::<_, PostCard>(&format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         WHERE p.status = 'published' \
         ORDER BY p.published_at DESC, p.id DESC \
         LIMIT $1 OFFSET $2"
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
    pool: &Pool,
    username: &str,
    limit: i64,
    offset: i64,
) -> Result<(Vec<PostCard>, i64), sqlx::Error> {
    let items = sqlx::query_as::<_, PostCard>(&format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         WHERE p.status = 'published' AND u.username = $1 \
         ORDER BY p.published_at DESC, p.id DESC \
         LIMIT $2 OFFSET $3"
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
        WHERE p.status = 'published' AND u.username = $1
        "#,
    )
    .bind(username)
    .fetch_one(pool)
    .await?;

    Ok((items, total))
}

pub async fn featured_posts_db(pool: &Pool, limit: i64) -> Result<Vec<PostCard>, sqlx::Error> {
    sqlx::query_as::<_, PostCard>(&format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         LEFT JOIN (SELECT post_id, COUNT(*) AS views FROM post_views GROUP BY post_id) v \
           ON v.post_id = p.id \
         WHERE p.status = 'published' \
         ORDER BY COALESCE(v.views, 0) DESC, p.published_at DESC, p.id DESC \
         LIMIT $1"
    ))
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn get_post_db(pool: &Pool, slug: &str) -> Result<Option<PostDetail>, sqlx::Error> {
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
        WHERE p.slug = $1
        "#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await
}

/// Returns the full edit-form data for a post (two queries: fields + tag ids).
pub async fn get_post_edit_db(pool: &Pool, id: i64) -> Result<PostEditData, sqlx::Error> {
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
             FROM posts WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    let tag_ids: Vec<i64> = sqlx::query_scalar("SELECT tag_id FROM post_tags WHERE post_id = $1")
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
    pool: &Pool,
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
    sqlx::query_scalar::<_, i64>(&format!(
        r#"
        INSERT INTO posts
          (title, slug, body_md, body_html, excerpt, author_id, category_id,
           featured_image_url, status, published_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9,
           CASE WHEN $10 = 'published' THEN {NOW} ELSE NULL END)
        RETURNING id
        "#,
    ))
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
    pool: &Pool,
    id: i64,
    title: &str,
    body_md: &str,
    body_html: &str,
    excerpt: &str,
    category_id: Option<i64>,
    featured_image_url: Option<&str>,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(&format!(
        r#"
        UPDATE posts SET
          title = $1, body_md = $2, body_html = $3, excerpt = $4,
          category_id = $5, featured_image_url = $6, status = $7,
          published_at = CASE WHEN $8 = 'published' AND published_at IS NULL
                              THEN {NOW} ELSE published_at END,
          updated_at = {NOW}
        WHERE id = $9
        "#,
    ))
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
pub async fn delete_post_db(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for sql in [
        "DELETE FROM post_tags WHERE post_id = $1",
        "DELETE FROM comments WHERE post_id = $1",
        "DELETE FROM post_views WHERE post_id = $1",
        "DELETE FROM arium_resource_members WHERE kind = 'post' AND resource_id = $1",
        "DELETE FROM posts WHERE id = $1",
    ] {
        sqlx::query(sql).bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Replace a post's tags with `tag_ids` (delete-then-insert).
pub async fn set_post_tags_db(
    pool: &Pool,
    post_id: i64,
    tag_ids: &[i64],
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM post_tags WHERE post_id = $1")
        .bind(post_id)
        .execute(pool)
        .await?;
    for tid in tag_ids {
        sqlx::query(
            "INSERT INTO post_tags (post_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
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
    pool: &Pool,
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
         WHERE ($1 OR p.author_id = $2) AND ($3 IS NULL OR p.status = $4) \
         ORDER BY {order_by}"
    );
    sqlx::query_as::<_, PostCard>(&sql)
        .bind(is_admin)
        .bind(author_id)
        .bind(status_filter)
        .bind(status_filter)
        .fetch_all(pool)
        .await
}

/// Full-text search with optional facet filters. Backend-specific search shape:
/// SQLite uses the `posts_fts` FTS5 virtual table with `MATCH`/`rank`; Postgres
/// uses a `tsvector` column on `posts` with `@@`/`ts_rank`. The placeholder
/// layout is the same in both backends — only the static SQL fragments differ.
///
/// `fts_query` is the raw user query. For SQLite the caller passes the
/// FTS5-tokenised form (whitespace-split, terms quoted, prefix `*` added);
/// for Postgres the caller passes the raw query and `plainto_tsquery` handles
/// tokenisation. `date_offset` is a duration modifier (e.g. `"-1 day"`) chosen
/// from a server-side whitelist.
pub async fn search_posts_db(
    pool: &Pool,
    fts_query: &str,
    limit: i64,
    offset: i64,
    category_slug: Option<&str>,
    tag_slug: Option<&str>,
    date_offset: Option<&str>,
) -> Result<(Vec<PostCard>, i64), sqlx::Error> {
    // $1 is always the fts query; facets walk $2.. in order; LIMIT/OFFSET trail.
    let mut placeholder: usize = 2;
    let mut facets = String::new();
    if category_slug.is_some() {
        facets.push_str(&format!(
            " AND p.category_id = (SELECT id FROM categories WHERE slug = ${placeholder})"
        ));
        placeholder += 1;
    }
    if tag_slug.is_some() {
        facets.push_str(&format!(
            " AND EXISTS (SELECT 1 FROM post_tags pt JOIN tags t ON t.id = pt.tag_id \
             WHERE pt.post_id = p.id AND t.slug = ${placeholder})"
        ));
        placeholder += 1;
    }
    if date_offset.is_some() {
        let cutoff = dialect::now_offset(placeholder);
        facets.push_str(&format!(" AND p.published_at >= {cutoff}"));
        placeholder += 1;
    }

    #[cfg(feature = "sqlite")]
    let (from_join, where_match, order_by) = (
        format!("FROM posts_fts f JOIN posts p ON p.id = f.rowid {POST_CARD_JOINS}"),
        "WHERE posts_fts MATCH $1",
        "ORDER BY rank".to_string(),
    );
    #[cfg(feature = "postgres")]
    let (from_join, where_match, order_by) = (
        format!("FROM posts p {POST_CARD_JOINS}"),
        "WHERE p.search_tsv @@ plainto_tsquery('english', $1)",
        "ORDER BY ts_rank(p.search_tsv, plainto_tsquery('english', $1)) DESC".to_string(),
    );

    let limit_n = placeholder;
    let offset_n = placeholder + 1;
    let items_sql = format!(
        "SELECT {POST_CARD_COLUMNS} {from_join} {where_match} AND p.status = 'published'{facets} \
         {order_by} LIMIT ${limit_n} OFFSET ${offset_n}"
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

    #[cfg(feature = "sqlite")]
    let count_from = "FROM posts_fts f JOIN posts p ON p.id = f.rowid".to_string();
    #[cfg(feature = "postgres")]
    let count_from = "FROM posts p".to_string();
    let count_sql =
        format!("SELECT COUNT(*) {count_from} {where_match} AND p.status = 'published'{facets}");
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
