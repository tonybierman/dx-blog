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
pub mod posts;
pub mod search;
pub mod subscribers;
pub mod taxonomy;

#[cfg(feature = "server")]
pub type DbExtension = axum::Extension<arium_dioxus::pool::Pool>;

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
        Err(ServerFnError::new("You don't have permission for this action."))
    }
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

/// Generate a unique slug from a title, appending `-2`, `-3`, … on collision.
#[cfg(feature = "server")]
pub async fn unique_slug(
    pool: &arium_dioxus::pool::Pool,
    title: &str,
) -> Result<String, sqlx::Error> {
    let base = {
        let s = slug::slugify(title);
        if s.is_empty() {
            "post".to_string()
        } else {
            s
        }
    };
    let mut candidate = base.clone();
    let mut n = 2;
    loop {
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM posts WHERE slug = ?")
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
