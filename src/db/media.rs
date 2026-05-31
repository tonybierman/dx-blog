use crate::model::{MediaItem, PostCard, PostUsage};
use arium_dioxus::pool::Pool;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

pub async fn list_media_db(pool: &Pool) -> Result<Vec<MediaItem>, sqlx::Error> {
    sqlx::query_as::<_, MediaItem>(
        "SELECT id, filename, url, uploaded_by, created_at FROM media ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn insert_media_stub_db(
    pool: &Pool,
    filename: &str,
    uploaded_by: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO media (filename, url, uploaded_by) VALUES ($1, '', $2) RETURNING id",
    )
    .bind(filename)
    .bind(uploaded_by)
    .fetch_one(pool)
    .await
}

pub async fn update_media_url_db(pool: &Pool, id: i64, url: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE media SET url = $1 WHERE id = $2")
        .bind(url)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_media_created_at_db(
    pool: &Pool,
    id: i64,
) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    sqlx::query_scalar("SELECT created_at FROM media WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn get_media_row_db(pool: &Pool, id: i64) -> Result<Option<(String, i64)>, sqlx::Error> {
    sqlx::query_as("SELECT url, uploaded_by FROM media WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn delete_media_db(pool: &Pool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM media WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

// ---------------------------------------------------------------- renditions

/// Record one generated rendition (a row in `media_variants`).
#[allow(clippy::too_many_arguments)]
pub async fn insert_variant_db(
    pool: &Pool,
    media_id: i64,
    label: &str,
    format: &str,
    width: i64,
    height: i64,
    url: &str,
    bytes: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO media_variants (media_id, label, format, width, height, url, bytes) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(media_id)
    .bind(label)
    .bind(format)
    .bind(width)
    .bind(height)
    .bind(url)
    .bind(bytes)
    .execute(pool)
    .await?;
    Ok(())
}

/// The on-disk URLs of a media row's renditions, so the caller can unlink the
/// files (the rows themselves cascade with the parent on delete).
pub async fn variant_urls_for_media_db(
    pool: &Pool,
    media_id: i64,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT url FROM media_variants WHERE media_id = $1")
        .bind(media_id)
        .fetch_all(pool)
        .await
}

/// Local uploads that have no renditions yet — the work list for the backfill.
pub async fn media_without_variants_db(pool: &Pool) -> Result<Vec<(i64, String)>, sqlx::Error> {
    sqlx::query_as::<_, (i64, String)>(
        "SELECT m.id, m.url FROM media m \
         WHERE m.url LIKE '/uploads/%' \
           AND NOT EXISTS (SELECT 1 FROM media_variants v WHERE v.media_id = m.id) \
         ORDER BY m.id",
    )
    .fetch_all(pool)
    .await
}

/// Build `(avif_srcset, webp_srcset)` for a set of media URLs in one query.
/// Returns a map keyed by the media's canonical URL; URLs with no renditions are
/// simply absent. Each `srcset` is `"url1 320w, url2 640w, …"` ordered by width.
pub async fn srcsets_for_urls(
    pool: &Pool,
    urls: &[String],
) -> Result<HashMap<String, (Option<String>, Option<String>)>, sqlx::Error> {
    if urls.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = (1..=urls.len())
        .map(|n| format!("${n}"))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT m.url AS media_url, v.format AS format, v.width AS width, v.url AS url \
         FROM media_variants v JOIN media m ON m.id = v.media_id \
         WHERE m.url IN ({placeholders}) \
         ORDER BY m.url, v.width ASC"
    );
    let mut q = sqlx::query_as::<_, (String, String, i64, String)>(&sql);
    for u in urls {
        q = q.bind(u);
    }
    let rows = q.fetch_all(pool).await?;

    let mut webp: HashMap<String, Vec<String>> = HashMap::new();
    let mut avif: HashMap<String, Vec<String>> = HashMap::new();
    for (media_url, format, width, url) in rows {
        let entry = format!("{url} {width}w");
        match format.as_str() {
            "webp" => webp.entry(media_url).or_default().push(entry),
            "avif" => avif.entry(media_url).or_default().push(entry),
            _ => {}
        }
    }

    let mut out: HashMap<String, (Option<String>, Option<String>)> = HashMap::new();
    for u in urls {
        let a = avif.get(u).map(|v| v.join(", "));
        let w = webp.get(u).map(|v| v.join(", "));
        if a.is_some() || w.is_some() {
            out.insert(u.clone(), (a, w));
        }
    }
    Ok(out)
}

/// Fill the responsive `srcset` fields on each card whose featured image is a
/// local upload with renditions. External URLs and un-processed uploads are left
/// as-is (their plain `featured_image_url` still renders).
pub async fn attach_card_variants(pool: &Pool, cards: &mut [PostCard]) -> Result<(), sqlx::Error> {
    let urls: Vec<String> = cards
        .iter()
        .filter_map(|c| c.featured_image_url.clone())
        .filter(|u| u.starts_with("/uploads/"))
        .collect();
    if urls.is_empty() {
        return Ok(());
    }
    let map = srcsets_for_urls(pool, &urls).await?;
    for c in cards.iter_mut() {
        if let Some(u) = &c.featured_image_url {
            if let Some((avif, webp)) = map.get(u) {
                c.featured_srcset_avif = avif.clone();
                c.featured_srcset_webp = webp.clone();
            }
        }
    }
    Ok(())
}

/// Like [`attach_card_variants`], for a single post-detail's featured image.
pub async fn attach_detail_variants(
    pool: &Pool,
    detail: &mut crate::model::PostDetail,
) -> Result<(), sqlx::Error> {
    let Some(u) = detail
        .featured_image_url
        .clone()
        .filter(|u| u.starts_with("/uploads/"))
    else {
        return Ok(());
    };
    let map = srcsets_for_urls(pool, std::slice::from_ref(&u)).await?;
    if let Some((avif, webp)) = map.get(&u) {
        detail.featured_srcset_avif = avif.clone();
        detail.featured_srcset_webp = webp.clone();
    }
    Ok(())
}

// ---------------------------------------------------------------- usage tracking

/// Escape SQLite `LIKE` metacharacters so a media URL is matched literally
/// (sanitized filenames can contain `_`, which `LIKE` treats as a wildcard).
/// Pair with `ESCAPE '\'`.
fn like_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Posts that reference a given media URL — as the featured/cover image
/// (`kind = "cover"`) or inline in the body (`kind = "body"`). Usage is derived,
/// not stored, so it can't drift: a library-picked cover stores this exact URL,
/// and inline images embed it in the markdown body.
pub async fn media_usage_db(pool: &Pool, url: &str) -> Result<Vec<PostUsage>, sqlx::Error> {
    let like = format!("%{}%", like_escape(url));
    sqlx::query_as::<_, PostUsage>(
        "SELECT id, title, slug, status, \
                CASE WHEN featured_image_url = $1 THEN 'cover' ELSE 'body' END AS kind \
         FROM posts \
         WHERE featured_image_url = $1 OR body_md LIKE $2 ESCAPE '\\' \
         ORDER BY id DESC",
    )
    .bind(url)
    .bind(like)
    .fetch_all(pool)
    .await
}

/// Every post's `(featured_image_url, body_md)` — the corpus the media list scans
/// once to compute each item's usage count without a query per row.
pub async fn post_image_corpus_db(
    pool: &Pool,
) -> Result<Vec<(Option<String>, String)>, sqlx::Error> {
    sqlx::query_as::<_, (Option<String>, String)>("SELECT featured_image_url, body_md FROM posts")
        .fetch_all(pool)
        .await
}
