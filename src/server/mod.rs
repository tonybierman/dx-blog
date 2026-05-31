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
#[cfg(feature = "server")]
pub mod highlight;
#[cfg(feature = "server")]
pub mod images;
#[cfg(feature = "server")]
pub mod live;
pub mod live_data;
pub mod posts;
pub mod reactions;
pub mod search;
pub mod settings;
pub mod subscribers;
pub mod taxonomy;

// Tests use an in-memory SqlitePool + include_str! on the SQLite-dialect init
// migration; they're meaningful only for the sqlite backend.
#[cfg(all(test, feature = "server", feature = "sqlite"))]
mod tests;
// Postgres-only coverage: runs the migration stack against a live `DATABASE_URL`
// (the CI `pg-migrate` job provides one). Gated separately so the sqlite test
// job doesn't try to compile the postgres-specific PgPoolOptions imports.
#[cfg(all(test, feature = "server", feature = "postgres"))]
mod tests_postgres;

#[cfg(feature = "server")]
pub type DbExtension = axum::Extension<arium_dioxus::pool::Pool>;

/// Shared mailer installed by `arium_dioxus::install()` (the `mail` feature is
/// on). Server fns reach it via this extractor and `&mail.0`.
#[cfg(feature = "server")]
pub type MailExtension = axum::Extension<arium_dioxus::Mailer>;

/// Map any server-side error to a `ServerFnError` for return from server fns.
///
/// The real error is logged server-side but NOT sent to the client: a raw sqlx
/// error string leaks schema, table, and constraint detail to anyone who can hit
/// an endpoint. Callers that want a specific, safe message (validation, "not
/// found", …) build a `ServerFnError::new(...)` directly instead of routing
/// through `sfe`.
pub fn sfe<E: std::fmt::Display>(e: E) -> ServerFnError {
    tracing::error!(target: "server", "{e}");
    ServerFnError::new("Something went wrong. Please try again.")
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
///
/// Compiled for both the server (post storage, feeds, seed) and the wasm client
/// (the editor's in-browser live preview), so the preview a writer sees is the
/// byte-for-byte same pipeline that produces the stored `body_html`.
#[cfg(any(feature = "server", feature = "web"))]
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

/// Resolve a unique slug for `name` in `table`, then run `insert(slug)` — and if
/// a concurrent creator grabbed that same slug in the check-then-insert gap (a
/// `UNIQUE` violation on the slug column), recompute and retry a bounded number
/// of times. Without this, two simultaneous "create with the same title" calls
/// both see the slug free, and the loser's INSERT surfaces a raw 500 instead of
/// quietly landing on `-2`. Non-unique-violation errors propagate immediately.
#[cfg(feature = "server")]
pub async fn create_with_unique_slug<T, F, Fut>(
    pool: &arium_dioxus::pool::Pool,
    table: &str,
    name: &str,
    mut insert: F,
) -> Result<T, ServerFnError>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Result<T, sqlx::Error>>,
{
    const MAX_ATTEMPTS: usize = 5;
    for attempt in 0..MAX_ATTEMPTS {
        let slug = crate::db::unique_slug(pool, table, name)
            .await
            .map_err(sfe)?;
        match insert(slug).await {
            Ok(v) => return Ok(v),
            Err(e) => {
                let collided = e
                    .as_database_error()
                    .is_some_and(|d| d.is_unique_violation());
                if collided && attempt + 1 < MAX_ATTEMPTS {
                    continue;
                }
                return Err(sfe(e));
            }
        }
    }
    // The loop only falls through after MAX_ATTEMPTS unique-violation retries.
    Err(ServerFnError::new(
        "Couldn't allocate a unique slug; please retry.",
    ))
}
