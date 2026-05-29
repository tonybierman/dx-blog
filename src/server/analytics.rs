//! View tracking (public) + aggregate analytics (admin).

use dioxus::prelude::*;

use crate::model::{AnalyticsSummary, DailyViews, PostCard, ReferrerStat};

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

/// Top external referrers by view count (admin only). Empty/NULL referrers are
/// bucketed as "(direct)" so direct traffic still shows up.
#[get("/api/analytics/referrers", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn top_referrers() -> Result<Vec<ReferrerStat>> {
    require_perm(&auth, ANALYTICS_READ)?;
    let rows = sqlx::query_as::<_, ReferrerStat>(
        r#"
        SELECT
          CASE WHEN referrer IS NULL OR referrer = '' THEN '(direct)' ELSE referrer END AS referrer,
          COUNT(*) AS views
        FROM post_views
        GROUP BY referrer
        ORDER BY views DESC
        LIMIT 10
        "#,
    )
    .fetch_all(&db.0)
    .await
    .map_err(sfe)?;
    Ok(rows)
}

/// Daily view counts over the last 30 days (admin only). Only days that
/// actually recorded views are returned; the chart renders one bar per row.
#[get("/api/analytics/views-over-time", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn views_over_time() -> Result<Vec<DailyViews>> {
    require_perm(&auth, ANALYTICS_READ)?;
    let rows = sqlx::query_as::<_, DailyViews>(
        r#"
        SELECT date(viewed_at) AS day, COUNT(*) AS views
        FROM post_views
        WHERE viewed_at >= date('now', '-29 days')
        GROUP BY day
        ORDER BY day
        "#,
    )
    .fetch_all(&db.0)
    .await
    .map_err(sfe)?;
    Ok(rows)
}
