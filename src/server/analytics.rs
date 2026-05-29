//! View tracking (public) + aggregate analytics (admin).

use dioxus::prelude::*;

use crate::model::{AnalyticsSummary, PostCard};

#[cfg(feature = "server")]
use crate::auth_tokens::ANALYTICS_READ;
#[cfg(feature = "server")]
use crate::server::{require_perm, sfe, DbExtension};

/// Record a page view for a post. Called from the post detail page.
#[post("/api/view", db: DbExtension)]
pub async fn record_view(post_id: i64, referrer: Option<String>) -> Result<()> {
    sqlx::query("INSERT INTO post_views (post_id, referrer) VALUES (?, ?)")
        .bind(post_id)
        .bind(&referrer)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}

/// Aggregate counts for the dashboard / analytics tiles (admin only).
#[get("/api/analytics/summary", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn analytics_summary() -> Result<AnalyticsSummary> {
    require_perm(&auth, ANALYTICS_READ)?;
    let row = sqlx::query_as::<_, AnalyticsSummary>(
        r#"
        SELECT
          (SELECT COUNT(*) FROM posts) AS post_count,
          (SELECT COUNT(*) FROM posts WHERE status = 'published') AS published_count,
          (SELECT COUNT(*) FROM posts WHERE status = 'draft') AS draft_count,
          (SELECT COUNT(*) FROM comments) AS comment_count,
          (SELECT COUNT(*) FROM comments WHERE status = 'pending') AS pending_comment_count,
          (SELECT COUNT(*) FROM subscribers) AS subscriber_count,
          (SELECT COUNT(*) FROM post_views) AS view_count
        "#,
    )
    .fetch_one(&db.0)
    .await
    .map_err(sfe)?;
    Ok(row)
}

/// Top posts by view count (admin only).
#[get("/api/analytics/top-posts", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn top_posts() -> Result<Vec<PostCard>> {
    require_perm(&auth, ANALYTICS_READ)?;
    let rows = sqlx::query_as::<_, PostCard>(
        r#"
        SELECT p.id, p.title, p.slug, p.excerpt, p.featured_image_url,
               p.author_id,
               COALESCE(u.display_name, u.username) AS author_name,
               c.name AS category_name,
               p.status, p.published_at
        FROM posts p
        JOIN users u ON u.id = p.author_id
        LEFT JOIN categories c ON c.id = p.category_id
        JOIN (SELECT post_id, COUNT(*) AS views FROM post_views GROUP BY post_id) v
          ON v.post_id = p.id
        ORDER BY v.views DESC
        LIMIT 10
        "#,
    )
    .fetch_all(&db.0)
    .await
    .map_err(sfe)?;
    Ok(rows)
}
