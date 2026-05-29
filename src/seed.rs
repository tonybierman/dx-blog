//! Idempotent demo-data seeding. Runs at startup only when `DX_SEED=1` is set
//! and the `posts` table is empty.

#![cfg(feature = "server")]

use arium_dioxus::auth;
use arium_dioxus::pool::Pool;

use crate::auth_tokens::{ALL_TOKENS, MEDIA_UPLOAD, POSTS_WRITE};
use crate::server::render_markdown;

/// Tokens granted to the two demo (non-admin) authors. The admin gets the full
/// [`ALL_TOKENS`] set.
const AUTHOR_TOKENS: &[&str] = &[POSTS_WRITE, MEDIA_UPLOAD];

async fn grant_token(pool: &Pool, user_id: i64, token: &str) -> anyhow::Result<()> {
    sqlx::query("INSERT OR IGNORE INTO user_permissions (user_id, token) VALUES (?, ?)")
        .bind(user_id)
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

/// Create demo users/content if the blog is empty; a no-op otherwise.
pub async fn run_if_empty(pool: &Pool) -> anyhow::Result<()> {
    let posts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM posts")
        .fetch_one(pool)
        .await?;
    if posts > 0 {
        return Ok(());
    }
    println!("[seed] empty blog — seeding demo data");

    // --- Users -------------------------------------------------------------
    // The admin is fully privileged, so it must NEVER get the public, hardcoded
    // "password" the demo authors use. Take it from DX_SEED_ADMIN_PASSWORD, or
    // mint a random one (via SQLite's randomblob, no rand crate) and print it
    // once so the operator can sign in.
    let admin_password = match std::env::var("DX_SEED_ADMIN_PASSWORD") {
        Ok(p) if !p.trim().is_empty() => p,
        _ => {
            let generated: String = sqlx::query_scalar("SELECT lower(hex(randomblob(16)))")
                .fetch_one(pool)
                .await?;
            println!(
                "[seed] generated admin password for admin@example.com: {generated}\n\
                 [seed]   (set DX_SEED_ADMIN_PASSWORD to choose your own; shown only once)"
            );
            generated
        }
    };
    let admin = auth::create_password_user(pool, "admin@example.com", &admin_password).await?;
    auth::mark_email_verified(pool, admin).await?;
    auth::grant_role(pool, admin, auth::role::ADMIN).await?;
    for t in ALL_TOKENS {
        grant_token(pool, admin, t).await?;
    }

    // The demo authors hold write tokens (POSTS_WRITE/MEDIA_UPLOAD), so a known,
    // hardcoded password would be a foothold if this ever seeds a shared/staging
    // DB. In debug builds keep the convenient shared "password" for local demos;
    // in release require an explicit DX_SEED_DEMO_PASSWORD, else mint a random one
    // and print it once (same treatment the admin gets).
    let demo_password = match std::env::var("DX_SEED_DEMO_PASSWORD") {
        Ok(p) if !p.trim().is_empty() => p,
        _ if cfg!(debug_assertions) => "password".to_string(),
        _ => {
            let generated: String = sqlx::query_scalar("SELECT lower(hex(randomblob(16)))")
                .fetch_one(pool)
                .await?;
            println!(
                "[seed] generated demo-author password (ada@/linus@example.com): {generated}\n\
                 [seed]   (set DX_SEED_DEMO_PASSWORD to choose your own; shown only once)"
            );
            generated
        }
    };

    let mut authors = Vec::new();
    for email in ["ada@example.com", "linus@example.com"] {
        let uid = auth::create_password_user(pool, email, &demo_password).await?;
        auth::mark_email_verified(pool, uid).await?;
        for t in AUTHOR_TOKENS {
            grant_token(pool, uid, t).await?;
        }
        authors.push(uid);
    }
    // Author pool includes the admin so admin-authored posts exist too. Push
    // rather than hard-index `authors[0]/[1]` — the email list above drives the
    // length, so indexing would panic if it were ever trimmed to one entry.
    authors.push(admin);
    let author_ids = authors;

    // --- Categories --------------------------------------------------------
    let categories = [
        (
            "Engineering",
            "engineering",
            "Posts about building software.",
        ),
        ("Design", "design", "Visual and interaction design."),
        ("Product", "product", "Shipping and strategy."),
        ("Culture", "culture", "Team and ways of working."),
    ];
    let mut category_ids = Vec::new();
    for (name, slug, desc) in categories {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO categories (name, slug, description) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(name)
        .bind(slug)
        .bind(desc)
        .fetch_one(pool)
        .await?;
        category_ids.push(id);
    }

    // --- Tags --------------------------------------------------------------
    let tags = [
        "rust",
        "dioxus",
        "web",
        "sqlite",
        "ux",
        "performance",
        "async",
        "tooling",
        "career",
        "open-source",
    ];
    let mut tag_ids = Vec::new();
    for name in tags {
        let id: i64 =
            sqlx::query_scalar("INSERT INTO tags (name, slug) VALUES (?, ?) RETURNING id")
                .bind(name)
                .bind(name)
                .fetch_one(pool)
                .await?;
        tag_ids.push(id);
    }

    // --- Posts -------------------------------------------------------------
    let mut post_ids = Vec::new();
    for i in 0..20 {
        let n = i + 1;
        let author = author_ids[i % author_ids.len()];
        let category = category_ids[i % category_ids.len()];
        // ~30% drafts.
        let status = if i % 3 == 0 { "draft" } else { "published" };
        let title = format!("Demo Post {n}: {}", SAMPLE_TITLES[i % SAMPLE_TITLES.len()]);
        let slug = format!("demo-post-{n}");
        let body_md = format!(
            "This is **demo** post number {n}. It covers _{topic}_ with a short \
             example.\n\n## A subheading\n\n```rust\nfn main() {{ println!(\"hello from post {n}\"); }}\n```\n\n\
             - point one\n- point two\n- point three\n\n> A pull quote to make the layout sing.",
            n = n,
            topic = tags[i % tags.len()],
        );
        let body_html = render_markdown(&body_md);
        let excerpt = format!("A short demo article ({n}) about {}.", tags[i % tags.len()]);

        let pid: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO posts
              (title, slug, body_md, body_html, excerpt, author_id, category_id, status, published_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?,
              CASE WHEN ? = 'published' THEN datetime('now', ?) ELSE NULL END)
            RETURNING id
            "#,
        )
        .bind(&title)
        .bind(&slug)
        .bind(&body_md)
        .bind(&body_html)
        .bind(&excerpt)
        .bind(author)
        .bind(category)
        .bind(status)
        .bind(status)
        .bind(format!("-{} days", 20 - i)) // spread publish dates
        .fetch_one(pool)
        .await?;
        post_ids.push((pid, author));

        // 1–3 tags per post.
        for k in 0..(1 + (i % 3)) {
            let tid = tag_ids[(i + k) % tag_ids.len()];
            sqlx::query("INSERT OR IGNORE INTO post_tags (post_id, tag_id) VALUES (?, ?)")
                .bind(pid)
                .bind(tid)
                .execute(pool)
                .await?;
        }

        // Author owns the post.
        sqlx::query(
            "INSERT INTO arium_resource_members (kind, resource_id, user_id, role)
             VALUES ('post', ?, ?, 'owner')
             ON CONFLICT (kind, resource_id, user_id) DO UPDATE SET role = excluded.role",
        )
        .bind(pid)
        .bind(author)
        .execute(pool)
        .await?;
    }

    // --- "Rust MDX" demo posts ---------------------------------------------
    // One post per live embeddable component, so each `[[component:…]]` block can
    // be opened on its own page. The prose renders through the normal pipeline;
    // `body_html` is still populated for feeds/RSS/SEO, which ignore the
    // interactive blocks.
    let mdx_demos: &[(&str, &str, &str, &str)] = &[
        (
            "rust-mdx-counter",
            "Rust MDX: a reactive counter",
            "A signal-driven counter mounted straight from Markdown — state lives in Rust, not an iframe.",
            "\
This counter is a **real Dioxus component** mounted from Markdown. Its state \
lives in a Rust signal compiled to WASM — no iframe.

[[component:counter start=0 step=1 label=\"Clicks\"]]

> Same Rust, same types, client and server — that's the point.",
        ),
        (
            "rust-mdx-chart",
            "Rust MDX: an inline SVG chart",
            "A chart rendered from inline data with hand-rolled SVG — no charting library.",
            "\
No charting library here — just hand-rolled SVG generated from inline data.

[[component:chart data=\"3,7,2,9,5,8,4\" kind=\"bar\" color=\"#6366f1\"]]

The same block renders a line instead:

[[component:chart data=\"3,7,2,9,5,8,4\" kind=\"line\" color=\"#22d3ee\"]]",
        ),
        (
            "rust-mdx-tweak",
            "Rust MDX: a tweakable visualization",
            "Drag a slider and watch the curve recompute live in the browser.",
            "\
Drag the slider; the curve recomputes live in WASM on every input.

[[component:tweak label=\"Frequency\"]]",
        ),
        (
            "rust-mdx-livechart",
            "Rust MDX: a live data feed",
            "New samples stream in on a timer and the window scrolls — all client-side.",
            "\
New samples stream in on a timer and the window scrolls left — entirely \
client-side, starting once the page hydrates.

[[component:livechart interval=800 window=28 color=\"#22d3ee\" label=\"Live feed\"]]",
        ),
        (
            "rust-mdx-stockchart",
            "Rust MDX: a live trading ticker",
            "A candlestick chart with volume that moves as the market \"opens\".",
            "\
Candles (with volume bars below) print on the right as the market \"opens\" — \
the price ticks and the session change flips green/red.

[[component:stockchart symbol=\"ACME\" interval=900 window=36 start=128]]",
        ),
    ];

    for (i, (slug, title, excerpt, body_md)) in mdx_demos.iter().enumerate() {
        let author = author_ids[i % author_ids.len()];
        let category = category_ids[i % category_ids.len()];
        let body_html = render_markdown(body_md);

        let pid: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO posts
              (title, slug, body_md, body_html, excerpt, author_id, category_id, status, published_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, 'published', datetime('now', ?))
            RETURNING id
            "#,
        )
        .bind(title)
        .bind(slug)
        .bind(body_md)
        .bind(&body_html)
        .bind(excerpt)
        .bind(author)
        .bind(category)
        .bind(format!("-{i} minutes")) // keep a stable order in the feed
        .fetch_one(pool)
        .await?;
        post_ids.push((pid, author));

        sqlx::query(
            "INSERT INTO arium_resource_members (kind, resource_id, user_id, role)
             VALUES ('post', ?, ?, 'owner')
             ON CONFLICT (kind, resource_id, user_id) DO UPDATE SET role = excluded.role",
        )
        .bind(pid)
        .bind(author)
        .execute(pool)
        .await?;
    }

    // --- Comments (30, mix of guest/user & pending/approved) ---------------
    for i in 0..30 {
        let (pid, _) = post_ids[i % post_ids.len()];
        let approved = i % 2 == 0;
        let status = if approved { "approved" } else { "pending" };
        if i % 3 == 0 {
            // guest comment
            sqlx::query(
                "INSERT INTO comments (post_id, guest_name, guest_email, body, status)
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(pid)
            .bind(format!("Guest {i}"))
            .bind(format!("guest{i}@example.com"))
            .bind(format!("Great write-up — comment #{i}!"))
            .bind(status)
            .execute(pool)
            .await?;
        } else {
            let commenter = author_ids[i % author_ids.len()];
            sqlx::query(
                "INSERT INTO comments (post_id, author_id, body, status) VALUES (?, ?, ?, ?)",
            )
            .bind(pid)
            .bind(commenter)
            .bind(format!("Thanks for sharing — comment #{i}."))
            .bind(status)
            .execute(pool)
            .await?;
        }
    }

    // --- Subscribers -------------------------------------------------------
    for i in 1..=5 {
        sqlx::query("INSERT OR IGNORE INTO subscribers (email, confirmed) VALUES (?, ?)")
            .bind(format!("subscriber{i}@example.com"))
            .bind((i % 2 == 0) as i64)
            .execute(pool)
            .await?;
    }

    println!(
        "[seed] done: 3 users, 4 categories, 10 tags, 25 posts (incl. 5 Rust MDX demos), \
         30 comments, 5 subscribers"
    );
    Ok(())
}

const SAMPLE_TITLES: &[&str] = &[
    "Getting Started",
    "Lessons Learned",
    "A Deep Dive",
    "Patterns That Scale",
    "Notes from the Field",
];
