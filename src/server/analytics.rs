//! View tracking (public) + aggregate analytics (admin).

use dioxus::prelude::*;

use crate::model::{AnalyticsSummary, DailyViews, PostCard, ReferrerStat};

#[cfg(feature = "server")]
use crate::auth_tokens::ANALYTICS_READ;
#[cfg(feature = "server")]
use crate::server::{require_perm, sfe, DbExtension, POST_CARD_COLUMNS, POST_CARD_JOINS};

/// Derive a coarse, privacy-preserving visitor fingerprint from request headers:
/// a hash of the forwarded client IP (falling back to user-agent). It's only
/// used to dedup views — not stored in the clear — so a stable per-process hash
/// (`DefaultHasher`'s fixed keys) is enough; we don't need a cryptographic one.
#[cfg(feature = "server")]
pub(crate) fn visitor_hash(headers: &axum::http::HeaderMap) -> String {
    use std::hash::{Hash, Hasher};
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    };
    // Behind a proxy the real client is the first X-Forwarded-For hop; otherwise
    // try X-Real-IP. User-agent is the last resort so direct hits still vary.
    let ip = {
        let first = header("x-forwarded-for")
            .split(',')
            .next()
            .unwrap_or("")
            .trim();
        if first.is_empty() {
            header("x-real-ip")
        } else {
            first
        }
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    ip.hash(&mut hasher);
    header("user-agent").hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Record a page view for a post. Called from the post detail page.
///
/// Public and unauthenticated, so it validates server-side that `post_id` refers
/// to a real *published* post — the conditional INSERT records nothing otherwise.
/// This stops anyone from POSTing arbitrary ids in a loop to inflate the
/// view-ranked "Featured"/"Top posts" lists and the analytics dashboard with rows
/// that don't correspond to any visible post.
///
/// It also dedups: a given visitor (see [`visitor_hash`]) counts at most once per
/// post per 24h, so refreshing or looping POSTs from one client no longer inflate
/// the count. The hash is persisted in the `visitor_hash` column the schema
/// reserved for exactly this.
#[post("/api/view", db: DbExtension, headers: axum::http::HeaderMap)]
pub async fn record_view(post_id: i64, referrer: Option<String>) -> Result<()> {
    let visitor = visitor_hash(&headers);
    // Cap the stored referrer: it's attacker-controlled (client-sent), shown
    // verbatim to admins, and never otherwise trusted — so bound its length to
    // keep a crafted value from bloating the table. Empty → NULL ("(direct)").
    const MAX_REFERRER_LEN: usize = 512;
    let referrer = referrer
        .map(|r| r.trim().chars().take(MAX_REFERRER_LEN).collect::<String>())
        .filter(|r| !r.is_empty());
    sqlx::query(
        "INSERT INTO post_views (post_id, referrer, visitor_hash)
         SELECT ?, ?, ?
         WHERE EXISTS (SELECT 1 FROM posts WHERE id = ? AND status = 'published')
           AND NOT EXISTS (
             SELECT 1 FROM post_views
             WHERE post_id = ? AND visitor_hash = ?
               AND viewed_at >= datetime('now', '-1 day'))",
    )
    .bind(post_id)
    .bind(&referrer)
    .bind(&visitor)
    .bind(post_id)
    .bind(post_id)
    .bind(&visitor)
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
    let rows = sqlx::query_as::<_, PostCard>(&format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         JOIN (SELECT post_id, COUNT(*) AS views FROM post_views GROUP BY post_id) v \
           ON v.post_id = p.id \
         ORDER BY v.views DESC \
         LIMIT 10"
    ))
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
