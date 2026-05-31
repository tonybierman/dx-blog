//! Dialect-divergent SQL fragments. Selected at compile time by the active
//! `sqlite` / `postgres` feature. The query layer is otherwise driver-agnostic
//! — these helpers cover the places where the two dialects disagree on syntax
//! for the same operation.
//!
//! Placeholder syntax is unified on `$N` (Postgres-style): SQLite accepts it
//! too (`$VVV` is a valid SQLite named-parameter sigil, sqlx-sqlite binds
//! positionally; see the `sqlx_sqlite_accepts_dollar_placeholders` test), so
//! no per-call gating is needed for that. Other divergences live here.

/// Server-side "now" expression. `datetime('now')` in SQLite returns an ISO
/// 8601 string (with a space, no timezone); `NOW()` in Postgres returns a
/// `TIMESTAMPTZ`. Interpolate directly into the SQL string at write sites:
///
/// ```ignore
/// use crate::db::dialect::NOW;
/// let sql = format!("UPDATE posts SET updated_at = {NOW} WHERE id = $1");
/// ```
pub const NOW: &str = if cfg!(feature = "postgres") {
    "NOW()"
} else {
    "datetime('now')"
};

/// 16 random bytes formatted as a 32-char lowercase hex string. Used to mint
/// subscriber-confirmation tokens. Postgres needs the `pgcrypto` extension
/// (installed by `migrations/postgres/20260601000000_blog_init.sql`).
///
/// ```ignore
/// use crate::db::dialect::RANDOM_HEX_16;
/// let sql = format!("SELECT {RANDOM_HEX_16}");
/// ```
pub const RANDOM_HEX_16: &str = if cfg!(feature = "postgres") {
    "encode(gen_random_bytes(16), 'hex')"
} else {
    "lower(hex(randomblob(16)))"
};

/// "Now + bound offset" expression at the given placeholder index. The same
/// bind value (e.g. `"-24 hours"`, `"-1 day"`, `"-15 minutes"`) works for both
/// backends — SQLite reads it as a `datetime()` modifier, Postgres casts it to
/// `INTERVAL`. Returns parenthesized so it composes cleanly inside comparisons.
///
/// ```ignore
/// use crate::db::dialect::now_offset;
/// let cutoff = now_offset(2);
/// let sql = format!(
///     "SELECT 1 FROM comments WHERE post_id = $1 AND created_at >= {cutoff}"
/// );
/// // .bind(post_id).bind("-30 seconds")
/// ```
pub fn now_offset(placeholder: usize) -> String {
    #[cfg(feature = "postgres")]
    {
        format!("(NOW() + ${placeholder}::interval)")
    }
    #[cfg(feature = "sqlite")]
    {
        format!("datetime('now', ${placeholder})")
    }
}
