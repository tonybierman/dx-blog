use crate::db::dialect;
use crate::db::{POST_CARD_COLUMNS, POST_CARD_JOINS};
use crate::model::{AnalyticsSummary, DailyViews, PostCard, ReferrerStat};
use arium_dioxus::pool::Pool;

pub async fn insert_view_db(
    pool: &Pool,
    post_id: i64,
    referrer: Option<&str>,
    visitor_hash: &str,
) -> Result<(), sqlx::Error> {
    let cutoff = dialect::now_offset(7);
    sqlx::query(&format!(
        "INSERT INTO post_views (post_id, referrer, visitor_hash)
         SELECT $1, $2, $3
         WHERE EXISTS (SELECT 1 FROM posts WHERE id = $4 AND status = 'published')
           AND NOT EXISTS (
             SELECT 1 FROM post_views
             WHERE post_id = $5 AND visitor_hash = $6
               AND viewed_at >= {cutoff})",
    ))
    .bind(post_id)
    .bind(referrer)
    .bind(visitor_hash)
    .bind(post_id)
    .bind(post_id)
    .bind(visitor_hash)
    .bind("-1 day")
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn analytics_summary_db(pool: &Pool) -> Result<AnalyticsSummary, sqlx::Error> {
    sqlx::query_as::<_, AnalyticsSummary>(
        r#"
        SELECT
          (SELECT COUNT(*) FROM posts) AS post_count,
          (SELECT COUNT(*) FROM posts WHERE status = 'published') AS published_count,
          (SELECT COUNT(*) FROM posts WHERE status = 'draft') AS draft_count,
          (SELECT COUNT(*) FROM comments) AS comment_count,
          (SELECT COUNT(*) FROM comments WHERE status = 'pending') AS pending_comment_count,
          (SELECT COUNT(*) FROM subscribers) AS subscriber_count,
          (SELECT COUNT(*) FROM post_views) AS view_count,
          (SELECT COUNT(*) FROM reactions) AS reaction_count
        "#,
    )
    .fetch_one(pool)
    .await
}

pub async fn top_posts_db(pool: &Pool) -> Result<Vec<PostCard>, sqlx::Error> {
    sqlx::query_as::<_, PostCard>(&format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         JOIN (SELECT post_id, COUNT(*) AS views FROM post_views GROUP BY post_id) v \
           ON v.post_id = p.id \
         ORDER BY v.views DESC \
         LIMIT 10"
    ))
    .fetch_all(pool)
    .await
}

pub async fn top_referrers_db(pool: &Pool) -> Result<Vec<ReferrerStat>, sqlx::Error> {
    sqlx::query_as::<_, ReferrerStat>(
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
    .fetch_all(pool)
    .await
}

pub async fn views_over_time_db(pool: &Pool) -> Result<Vec<DailyViews>, sqlx::Error> {
    // `date()` and `date('now', '-29 days')` are SQLite-specific; Postgres uses
    // a cast (`::date`) and `CURRENT_DATE - INTERVAL '29 days'`.
    #[cfg(feature = "sqlite")]
    let sql = r#"
        SELECT date(viewed_at) AS day, COUNT(*) AS views
        FROM post_views
        WHERE viewed_at >= date('now', '-29 days')
        GROUP BY day
        ORDER BY day
    "#;
    #[cfg(feature = "postgres")]
    let sql = r#"
        SELECT viewed_at::date AS day, COUNT(*) AS views
        FROM post_views
        WHERE viewed_at >= CURRENT_DATE - INTERVAL '29 days'
        GROUP BY day
        ORDER BY day
    "#;
    sqlx::query_as::<_, DailyViews>(sql).fetch_all(pool).await
}
