//! Postgres-only integration test. Requires a live Postgres reachable at
//! `DATABASE_URL` — CI spins one up via a `services: postgres` block; locally,
//! point it at a throwaway container, e.g.:
//!
//! ```text
//! docker run -d --rm --name pg-test \
//!   -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=riparion_test \
//!   -p 5432:5432 postgres:16-alpine
//! DATABASE_URL=postgres://postgres:postgres@localhost:5432/riparion_test \
//!   cargo test --no-default-features --features server,postgres
//! ```
//!
//! The SQLite path's tests are in `tests.rs` (in-memory `:memory:` pool); they
//! aren't meaningful here because they're built against `SqliteConnectOptions`.

use sqlx::postgres::PgPoolOptions;

/// Runs every migrator the app boots with — arium core, arium membership, and
/// riparion's blog init — against a live Postgres, in the same order `main.rs`
/// does. Catches dialect SQL regressions that compile-time linting can't see.
#[tokio::test]
async fn pg_migrations_apply() {
    let url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set (pg-migrate job exports it)");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("connect to postgres");

    arium_dioxus::migrator()
        .run(&pool)
        .await
        .expect("arium core migrator");
    arium_dioxus::membership_migrator()
        .run(&pool)
        .await
        .expect("arium membership migrator");
    {
        let mut m = sqlx::migrate!("./migrations/postgres");
        m.set_ignore_missing(true);
        m.run(&pool).await.expect("riparion blog migrator");
    }

    // arium occupies versions 1..=9; riparion adds at least 20260601000000.
    // Asserting >=10 rows guards against a migrator silently skipping itself
    // (e.g. a future "ignore everything" misconfiguration).
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .expect("count migrations");
    assert!(
        count >= 10,
        "expected 9 arium + ≥1 blog migration rows, got {count}"
    );

    // Sanity-probe that the FTS trigger function exists — its DDL is the most
    // dialect-specific piece of the blog init migration and most likely to
    // regress under an idempotent re-run or schema rewrite.
    let has_trigger: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_trigger WHERE tgname = 'posts_search_tsv_trg')",
    )
    .fetch_one(&pool)
    .await
    .expect("probe FTS trigger");
    assert!(
        has_trigger,
        "FTS tsvector trigger missing — postgres init migration likely regressed"
    );
}
