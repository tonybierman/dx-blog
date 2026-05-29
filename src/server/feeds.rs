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

use axum::{http::header, response::IntoResponse};

use crate::server::DbExtension;

/// Number of most-recent posts included in the Atom feed.
const FEED_LIMIT: i64 = 20;

/// Canonical site origin for absolute URLs, e.g. `https://blog.example.com`.
/// Reads `SITE_URL` (trailing slash trimmed); falls back to localhost in dev.
pub fn site_base() -> String {
    std::env::var("SITE_URL")
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://localhost:3000".to_string())
}

/// Human-facing site name for the feed `<title>`. Reads `SITE_TITLE`.
fn site_title() -> String {
    std::env::var("SITE_TITLE")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "dx-blog".to_string())
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

/// SQLite stores `datetime('now')` as `YYYY-MM-DD HH:MM:SS` in UTC. Both Atom
/// (RFC 3339) and the sitemap (W3C datetime) accept `YYYY-MM-DDTHH:MM:SSZ`, so
/// the conversion is just a separator swap plus a UTC marker. Values already
/// containing a `T` are passed through unchanged.
fn to_rfc3339(dt: &str) -> String {
    let t = dt.trim();
    if t.is_empty() || t.contains('T') {
        return t.to_string();
    }
    format!("{}Z", t.replacen(' ', "T", 1))
}

/// `GET /sitemap.xml` — the home and archive landing pages, every published
/// post, plus each category / tag / author index that has published content.
pub async fn sitemap_handler(db: DbExtension) -> impl IntoResponse {
    let pool = &db.0;
    let base = site_base();

    // (loc, optional lastmod). Static landing pages have no meaningful lastmod.
    let mut urls: Vec<(String, Option<String>)> = vec![
        (format!("{base}/"), None),
        (format!("{base}/archive"), None),
    ];

    // Published posts — lastmod is the most recent of updated/published/created.
    let posts = sqlx::query_as::<_, (String, String)>(
        r#"
        SELECT slug, COALESCE(updated_at, published_at, created_at) AS lastmod
        FROM posts
        WHERE status = 'published'
        ORDER BY lastmod DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    for (slug, lastmod) in posts {
        urls.push((format!("{base}/post/{slug}"), Some(to_rfc3339(&lastmod))));
    }

    // Category and tag index pages.
    for (sql, prefix) in [
        ("SELECT slug FROM categories ORDER BY slug", "category"),
        ("SELECT slug FROM tags ORDER BY slug", "tag"),
    ] {
        let slugs = sqlx::query_scalar::<_, String>(sql)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
        for slug in slugs {
            urls.push((format!("{base}/{prefix}/{slug}"), None));
        }
    }

    // Authors with at least one published post.
    let authors = sqlx::query_scalar::<_, String>(
        r#"
        SELECT DISTINCT u.username
        FROM users u
        JOIN posts p ON p.author_id = u.id
        WHERE p.status = 'published'
        ORDER BY u.username
        "#,
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
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

    (
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        body,
    )
}

#[derive(sqlx::FromRow)]
struct FeedRow {
    title: String,
    slug: String,
    excerpt: String,
    body_html: String,
    published_at: Option<String>,
    updated_at: String,
    author_name: String,
}

/// `GET /feed.xml` — an Atom 1.0 feed of the most recent published posts, with
/// the full rendered HTML carried in each entry's `<content>`.
pub async fn atom_handler(db: DbExtension) -> impl IntoResponse {
    let pool = &db.0;
    let base = site_base();
    let title = site_title();
    let feed_url = format!("{base}/feed.xml");

    let posts = sqlx::query_as::<_, FeedRow>(
        r#"
        SELECT p.title, p.slug, p.excerpt, p.body_html,
               p.published_at, p.updated_at,
               COALESCE(u.display_name, u.username) AS author_name
        FROM posts p
        JOIN users u ON u.id = p.author_id
        WHERE p.status = 'published'
        ORDER BY p.published_at DESC, p.id DESC
        LIMIT ?
        "#,
    )
    .bind(FEED_LIMIT)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Feed-level <updated> = newest entry's timestamp (updated, else published).
    // Atom requires the element even for an empty feed, so fall back to epoch.
    let feed_updated = posts
        .first()
        .map(|p| {
            let raw = if !p.updated_at.trim().is_empty() {
                p.updated_at.as_str()
            } else {
                p.published_at.as_deref().unwrap_or("")
            };
            to_rfc3339(raw)
        })
        .filter(|s| !s.is_empty())
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
        let published = p
            .published_at
            .as_deref()
            .map(to_rfc3339)
            .unwrap_or_default();
        let updated = {
            let u = to_rfc3339(&p.updated_at);
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

    (
        [(header::CONTENT_TYPE, "application/atom+xml; charset=utf-8")],
        body,
    )
}
