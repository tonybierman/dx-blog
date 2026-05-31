//! Public XML endpoints: `/sitemap.xml` (for crawlers) and `/feed.xml` (an Atom
//! feed for readers).
//!
//! These are plain axum GET handlers, not Dioxus server functions — they emit
//! raw XML with their own content-type, which the server-fn wire format can't
//! express. They're registered on the router in `main.rs` and pick up the
//! shared pool from the `Extension<Pool>` that `arium_dioxus::install()` layers
//! over the whole router.
//!
//! Atom (not RSS) is the chosen feed format: its timestamps are RFC 3339, which
//! is a trivial reshaping of SQLite's `datetime('now')` output, whereas RSS's
//! RFC 822 dates would need weekday/month-name computation (i.e. a date crate).
//! Atom is universally supported by feed readers, so nothing is lost.

#![cfg(feature = "server")]

use axum::{
    http::{header, StatusCode},
    response::IntoResponse,
};

use crate::db::feeds::{
    feed_active_author_usernames_db, feed_atom_posts_db, feed_category_slugs_db,
    feed_published_posts_db, feed_tag_slugs_db,
};
use crate::model::to_rfc3339;
use crate::server::DbExtension;

/// Number of most-recent posts included in the Atom feed.
const FEED_LIMIT: i64 = 20;

/// True if `s` looks like an `http(s)://host[:port]` origin: a known scheme and
/// a non-empty host with no embedded whitespace or path. Guards against a
/// misconfigured `SITE_URL` (e.g. `blog.example.com`, or a value with a path)
/// being concatenated into og:url / feed / emailed-confirm links.
fn is_valid_origin(s: &str) -> bool {
    match s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
    {
        Some(host) => !host.is_empty() && !host.contains([' ', '\t', '/']),
        None => false,
    }
}

/// Canonical site origin for absolute URLs, e.g. `https://blog.example.com`.
/// Reads `SITE_URL` (trailing slash trimmed); falls back to localhost when unset
/// or when the value isn't a valid `http(s)://` origin.
pub fn site_base() -> String {
    std::env::var("SITE_URL")
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| is_valid_origin(s))
        .unwrap_or_else(|| "http://localhost:3000".to_string())
}

/// Human-facing site name for the feed `<title>`. Prefers the admin-configured
/// site title (the same value the page chrome shows) so the feed stays in sync
/// with the site; falls back to the `SITE_TITLE` env, then the shared default.
async fn site_title(pool: &arium_dioxus::pool::Pool) -> String {
    let from_db = crate::db::settings::get_setting_db(pool, "site_title")
        .await
        .ok()
        .flatten()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    from_db
        .or_else(|| {
            std::env::var("SITE_TITLE")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| crate::server::settings::DEFAULT_SITE_TITLE.to_string())
}

/// XML-escape text for safe inclusion in element content or attributes.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// `GET /sitemap.xml` — the home and archive landing pages, every published
/// post, plus each category / tag / author index that has published content.
pub async fn sitemap_handler(db: DbExtension) -> Result<impl IntoResponse, StatusCode> {
    let pool = &db.0;
    let base = site_base();

    // (loc, optional lastmod). Static landing pages have no meaningful lastmod.
    let mut urls: Vec<(String, Option<String>)> = vec![
        (format!("{base}/"), None),
        (format!("{base}/archive"), None),
    ];

    let posts = feed_published_posts_db(pool).await.map_err(|e| {
        tracing::warn!(target: "sitemap", "posts query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    for (slug, lastmod) in posts {
        urls.push((
            format!("{base}/post/{slug}"),
            lastmod.as_ref().map(to_rfc3339),
        ));
    }

    let category_slugs = feed_category_slugs_db(pool).await.map_err(|e| {
        tracing::warn!(target: "sitemap", "category slugs query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    for slug in category_slugs {
        urls.push((format!("{base}/category/{slug}"), None));
    }

    let tag_slugs = feed_tag_slugs_db(pool).await.map_err(|e| {
        tracing::warn!(target: "sitemap", "tag slugs query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    for slug in tag_slugs {
        urls.push((format!("{base}/tag/{slug}"), None));
    }

    let authors = feed_active_author_usernames_db(pool).await.map_err(|e| {
        tracing::warn!(target: "sitemap", "authors query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    for username in authors {
        urls.push((format!("{base}/author/{username}"), None));
    }

    let mut body = String::new();
    body.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    body.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
    for (loc, lastmod) in urls {
        body.push_str("  <url><loc>");
        body.push_str(&xml_escape(&loc));
        body.push_str("</loc>");
        if let Some(m) = lastmod.filter(|m| !m.is_empty()) {
            body.push_str("<lastmod>");
            body.push_str(&xml_escape(&m));
            body.push_str("</lastmod>");
        }
        body.push_str("</url>\n");
    }
    body.push_str("</urlset>\n");

    Ok((
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        body,
    ))
}

/// `GET /feed.xml` — an Atom 1.0 feed of the most recent published posts, with
/// the full rendered HTML carried in each entry's `<content>`.
pub async fn atom_handler(db: DbExtension) -> Result<impl IntoResponse, StatusCode> {
    let pool = &db.0;
    let base = site_base();
    let title = site_title(pool).await;
    let feed_url = format!("{base}/feed.xml");

    let posts = feed_atom_posts_db(pool, FEED_LIMIT).await.map_err(|e| {
        tracing::warn!(target: "feed", "posts query failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Feed-level <updated> = newest entry's timestamp (updated, else published).
    // Atom requires the element even for an empty feed, so fall back to epoch.
    let feed_updated = posts
        .first()
        .and_then(|p| p.updated_at.as_ref().or(p.published_at.as_ref()))
        .map(to_rfc3339)
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());

    let mut body = String::new();
    body.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    body.push_str("<feed xmlns=\"http://www.w3.org/2005/Atom\">\n");
    body.push_str(&format!("  <title>{}</title>\n", xml_escape(&title)));
    body.push_str(&format!("  <link href=\"{}/\"/>\n", xml_escape(&base)));
    body.push_str(&format!(
        "  <link rel=\"self\" type=\"application/atom+xml\" href=\"{}\"/>\n",
        xml_escape(&feed_url)
    ));
    body.push_str(&format!("  <id>{}/</id>\n", xml_escape(&base)));
    body.push_str(&format!(
        "  <updated>{}</updated>\n",
        xml_escape(&feed_updated)
    ));

    for p in posts {
        let url = format!("{base}/post/{}", p.slug);
        let published = p.published_at.as_ref().map(to_rfc3339).unwrap_or_default();
        let updated = {
            let u = p.updated_at.as_ref().map(to_rfc3339).unwrap_or_default();
            if u.is_empty() {
                published.clone()
            } else {
                u
            }
        };

        body.push_str("  <entry>\n");
        body.push_str(&format!("    <title>{}</title>\n", xml_escape(&p.title)));
        body.push_str(&format!("    <link href=\"{}\"/>\n", xml_escape(&url)));
        body.push_str(&format!("    <id>{}</id>\n", xml_escape(&url)));
        if !published.is_empty() {
            body.push_str(&format!(
                "    <published>{}</published>\n",
                xml_escape(&published)
            ));
        }
        if !updated.is_empty() {
            body.push_str(&format!(
                "    <updated>{}</updated>\n",
                xml_escape(&updated)
            ));
        }
        body.push_str(&format!(
            "    <author><name>{}</name></author>\n",
            xml_escape(&p.author_name)
        ));
        if !p.excerpt.trim().is_empty() {
            body.push_str(&format!(
                "    <summary>{}</summary>\n",
                xml_escape(&p.excerpt)
            ));
        }
        body.push_str(&format!(
            "    <content type=\"html\">{}</content>\n",
            xml_escape(&p.body_html)
        ));
        body.push_str("  </entry>\n");
    }

    body.push_str("</feed>\n");

    Ok((
        [(header::CONTENT_TYPE, "application/atom+xml; charset=utf-8")],
        body,
    ))
}
