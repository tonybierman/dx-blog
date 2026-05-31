use crate::db::dialect;
use crate::model::{CommentView, RecentComment};
use arium_dioxus::pool::Pool;

pub const COMMENT_VIEW_COLUMNS: &str = "cm.id, cm.post_id, \
     COALESCE(u.display_name, u.username, cm.guest_name, 'Anonymous') AS display_name, \
     cm.body, cm.status, cm.created_at";

pub const COMMENT_VIEW_FROM: &str = "FROM comments cm LEFT JOIN users u ON u.id = cm.author_id";

pub async fn list_comments_db(pool: &Pool, post_id: i64) -> Result<Vec<CommentView>, sqlx::Error> {
    sqlx::query_as::<_, CommentView>(&format!(
        "SELECT {COMMENT_VIEW_COLUMNS} {COMMENT_VIEW_FROM} \
         WHERE cm.post_id = $1 AND cm.status = 'approved' \
         ORDER BY cm.created_at ASC"
    ))
    .bind(post_id)
    .fetch_all(pool)
    .await
}

pub async fn recent_comments_db(
    pool: &Pool,
    limit: i64,
) -> Result<Vec<RecentComment>, sqlx::Error> {
    sqlx::query_as::<_, RecentComment>(
        r#"
        SELECT cm.id, p.title AS post_title, p.slug AS post_slug,
               COALESCE(u.display_name, u.username, cm.guest_name, 'Anonymous') AS display_name,
               cm.body, cm.created_at
        FROM comments cm
        JOIN posts p ON p.id = cm.post_id AND p.status = 'published'
        LEFT JOIN users u ON u.id = cm.author_id
        WHERE cm.status = 'approved'
        ORDER BY cm.created_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn post_is_published_db(pool: &Pool, post_id: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM posts WHERE id = $1 AND status = 'published')")
        .bind(post_id)
        .fetch_one(pool)
        .await
}

/// Returns a sentinel row if the logged-in user already commented on this post
/// within `cooldown` (a SQLite datetime offset string).
pub async fn commenter_cooldown_uid_db(
    pool: &Pool,
    post_id: i64,
    author_id: i64,
    cooldown: &str,
) -> Result<Option<i64>, sqlx::Error> {
    let cutoff = dialect::now_offset(3);
    sqlx::query_scalar(&format!(
        "SELECT 1 FROM comments WHERE post_id = $1 AND author_id = $2 \
         AND created_at >= {cutoff} LIMIT 1",
    ))
    .bind(post_id)
    .bind(author_id)
    .bind(cooldown)
    .fetch_optional(pool)
    .await
}

/// Returns a sentinel row if the guest email already commented on this post
/// within `cooldown`.
pub async fn commenter_cooldown_email_db(
    pool: &Pool,
    post_id: i64,
    guest_email: &str,
    cooldown: &str,
) -> Result<Option<i64>, sqlx::Error> {
    let cutoff = dialect::now_offset(3);
    sqlx::query_scalar(&format!(
        "SELECT 1 FROM comments WHERE post_id = $1 AND guest_email = $2 \
         AND created_at >= {cutoff} LIMIT 1",
    ))
    .bind(post_id)
    .bind(guest_email)
    .bind(cooldown)
    .fetch_optional(pool)
    .await
}

pub async fn burst_count_db(
    pool: &Pool,
    post_id: i64,
    burst_window: &str,
) -> Result<i64, sqlx::Error> {
    let cutoff = dialect::now_offset(2);
    sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM comments WHERE post_id = $1 AND created_at >= {cutoff}",
    ))
    .bind(post_id)
    .bind(burst_window)
    .fetch_one(pool)
    .await
}

pub async fn has_prior_approved_comment_db(
    pool: &Pool,
    author_id: i64,
) -> Result<bool, sqlx::Error> {
    let row: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM comments WHERE author_id = $1 AND status = 'approved' LIMIT 1",
    )
    .bind(author_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

pub async fn insert_comment_db(
    pool: &Pool,
    post_id: i64,
    author_id: Option<i64>,
    guest_name: Option<&str>,
    guest_email: Option<&str>,
    body: &str,
    status: &str,
) -> Result<i64, sqlx::Error> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO comments (post_id, author_id, guest_name, guest_email, body, status)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(post_id)
    .bind(author_id)
    .bind(guest_name)
    .bind(guest_email)
    .bind(body)
    .bind(status)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn fetch_comment_by_id_db(pool: &Pool, id: i64) -> Result<CommentView, sqlx::Error> {
    sqlx::query_as::<_, CommentView>(&format!(
        "SELECT {COMMENT_VIEW_COLUMNS} {COMMENT_VIEW_FROM} WHERE cm.id = $1"
    ))
    .bind(id)
    .fetch_one(pool)
    .await
}

/// Notification metadata for one comment plus its post's title/slug, for the
/// site-wide admin live channel. Deliberately omits the comment body: the admin
/// stream carries metadata only (see [`crate::model::AdminEvent`]). One round
/// trip; no `published` filter, since admins moderate comments on any post.
#[derive(sqlx::FromRow)]
pub struct CommentWithPost {
    pub id: i64,
    pub post_id: i64,
    pub display_name: String,
    pub status: String,
    pub post_title: String,
    pub post_slug: String,
}

pub async fn comment_with_post_db(pool: &Pool, id: i64) -> Result<CommentWithPost, sqlx::Error> {
    sqlx::query_as::<_, CommentWithPost>(
        r#"
        SELECT cm.id, cm.post_id,
               COALESCE(u.display_name, u.username, cm.guest_name, 'Anonymous') AS display_name,
               cm.status, p.title AS post_title, p.slug AS post_slug
        FROM comments cm
        JOIN posts p ON p.id = cm.post_id
        LEFT JOIN users u ON u.id = cm.author_id
        WHERE cm.id = $1
        "#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
}

pub async fn list_comments_admin_db(
    pool: &Pool,
    status: Option<&str>,
) -> Result<Vec<CommentView>, sqlx::Error> {
    sqlx::query_as::<_, CommentView>(&format!(
        "SELECT {COMMENT_VIEW_COLUMNS} {COMMENT_VIEW_FROM} \
         WHERE ($1 IS NULL OR cm.status = $2) \
         ORDER BY cm.created_at DESC"
    ))
    .bind(status)
    .bind(status)
    .fetch_all(pool)
    .await
}

pub async fn moderate_comment_db(pool: &Pool, id: i64, status: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE comments SET status = $1 WHERE id = $2")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_comment_db(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM comments WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
