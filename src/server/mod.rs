//! Blog server functions and server-only helpers.
//!
//! The shared SQLite pool is installed by `arium_dioxus::install()` as an
//! `axum::Extension<Pool>`; server fns reach it via the `DbExtension` extractor
//! and `&db.0`.

use dioxus::prelude::*;

pub mod admin;
pub mod analytics;
pub mod authors;
pub mod comments;
#[cfg(feature = "server")]
pub mod feeds;
pub mod posts;
pub mod search;
pub mod settings;
pub mod subscribers;
pub mod taxonomy;

#[cfg(all(test, feature = "server"))]
mod tests;

#[cfg(feature = "server")]
pub type DbExtension = axum::Extension<arium_dioxus::pool::Pool>;

/// Shared mailer installed by `arium_dioxus::install()` (the `mail` feature is
/// on). Server fns reach it via this extractor and `&mail.0`.
#[cfg(feature = "server")]
pub type MailExtension = axum::Extension<arium_dioxus::Mailer>;

/// Map any server-side error to a `ServerFnError` for return from server fns.
pub fn sfe<E: std::fmt::Display>(e: E) -> ServerFnError {
    ServerFnError::new(e.to_string())
}

/// Require the current session to hold a global permission token. Returns the
/// signed-in user's id on success.
#[cfg(feature = "server")]
pub fn require_perm(
    auth: &arium_dioxus::auth::Session,
    token: &str,
) -> std::result::Result<i64, ServerFnError> {
    let user = auth
        .current_user
        .as_ref()
        .filter(|u| !u.anonymous)
        .ok_or_else(|| ServerFnError::new("Not signed in."))?;
    if user.permissions.contains(token) {
        Ok(user.id as i64)
    } else {
        Err(ServerFnError::new(
            "You don't have permission for this action.",
        ))
    }
}

/// A pragmatic email sanity check: a single `@` with a non-empty local part and
/// a dotted domain (non-empty labels on both sides of the last dot). Rejects the
/// likes of `"a@b"` and `"@x.com"`. Not a full RFC validator — for a subscriber
/// the confirmation email is the real proof. Shared by the subscribe flow and
/// guest-comment validation so both apply the same rule.
#[cfg(feature = "server")]
pub fn looks_like_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    if local.is_empty() || domain.contains('@') {
        return false;
    }
    matches!(domain.rsplit_once('.'), Some((host, tld)) if !host.is_empty() && !tld.is_empty())
}

/// Render Markdown source to sanitized HTML for storage/display.
#[cfg(feature = "server")]
pub fn render_markdown(md: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(md, options);
    let mut unsafe_html = String::new();
    html::push_html(&mut unsafe_html, parser);
    ammonia::clean(&unsafe_html)
}

/// The `PostCard` 10-column projection, shared across every feed/search/analytics
/// read path so a column change is made in one place. Compose with `format!`;
/// the `FROM` clause and bind order live in each caller.
#[cfg(feature = "server")]
pub const POST_CARD_COLUMNS: &str = "p.id, p.title, p.slug, p.excerpt, p.featured_image_url, \
     p.author_id, COALESCE(u.display_name, u.username) AS author_name, \
     c.name AS category_name, p.status, p.published_at";

/// The joins those columns depend on (author for `author_name`, category for
/// `category_name`). Placed right after a `FROM` that aliases the posts row as
/// `p` (the plain `FROM posts p`, or the FTS join in search).
#[cfg(feature = "server")]
pub const POST_CARD_JOINS: &str =
    "JOIN users u ON u.id = p.author_id LEFT JOIN categories c ON c.id = p.category_id";

/// Generate a unique slug for `name` within `table`, appending `-2`, `-3`, … on
/// collision. `table` is always an internal constant (`"posts"`, `"categories"`,
/// `"tags"`), never user input, so interpolating it into the query is safe.
#[cfg(feature = "server")]
pub async fn unique_slug(
    pool: &arium_dioxus::pool::Pool,
    table: &str,
    name: &str,
) -> Result<String, sqlx::Error> {
    let base = {
        let s = slug::slugify(name);
        if s.is_empty() {
            "item".to_string()
        } else {
            s
        }
    };
    let mut candidate = base.clone();
    let mut n = 2;
    let sql = format!("SELECT id FROM {table} WHERE slug = ?");
    loop {
        let exists: Option<i64> = sqlx::query_scalar(&sql)
            .bind(&candidate)
            .fetch_optional(pool)
            .await?;
        if exists.is_none() {
            return Ok(candidate);
        }
        candidate = format!("{base}-{n}");
        n += 1;
    }
}
