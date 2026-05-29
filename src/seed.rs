//! Idempotent demo-data seeding. Runs automatically at startup when the `posts`
//! table is empty. Creates the fixtures listed in PLAN.md.

#![cfg(feature = "server")]

use arium_dioxus::auth;
use arium_dioxus::pool::Pool;

use crate::auth_tokens::{
    ANALYTICS_READ, COMMENTS_MODERATE, MEDIA_UPLOAD, POSTS_WRITE, POSTS_WRITE_ANY, SETTINGS_WRITE,
    USERS_MANAGE,
};
use crate::server::render_markdown;

const ALL_TOKENS: &[&str] = &[
    POSTS_WRITE,
    POSTS_WRITE_ANY,
    MEDIA_UPLOAD,
    COMMENTS_MODERATE,
    USERS_MANAGE,
    SETTINGS_WRITE,
    ANALYTICS_READ,
];
const AUTHOR_TOKENS: &[&str] = &[POSTS_WRITE, MEDIA_UPLOAD];

async fn grant_token(pool: &Pool, user_id: i64, token: &str) -> anyhow::Result<()> {
    sqlx::query("INSERT OR IGNORE INTO user_permissions (user_id, token) VALUES (?, ?)")
        .bind(user_id)
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

/// Create demo users/content if the blog is empty. Safe to call on every boot.
pub async fn run_if_empty(pool: &Pool) -> anyhow::Result<()> {
    let posts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM posts")
        .fetch_one(pool)
        .await?;
    if posts > 0 {
        return Ok(());
    }
    println!("[seed] empty blog — seeding demo data (password for every account: 'password')");

    // --- Users -------------------------------------------------------------
    let admin = auth::create_password_user(pool, "admin@example.com", "password").await?;
    auth::mark_email_verified(pool, admin).await?;
    auth::grant_role(pool, admin, auth::role::ADMIN).await?;
    for t in ALL_TOKENS {
        grant_token(pool, admin, t).await?;
    }

    let mut authors = Vec::new();
    for email in ["ada@example.com", "linus@example.com"] {
        let uid = auth::create_password_user(pool, email, "password").await?;
        auth::mark_email_verified(pool, uid).await?;
        for t in AUTHOR_TOKENS {
            grant_token(pool, uid, t).await?;
        }
        authors.push(uid);
    }
    // Author pool includes the admin so admin-authored posts exist too.
    let author_ids = [authors[0], authors[1], admin];

    // --- Categories --------------------------------------------------------
    let categories = [
        ("Engineering", "engineering", "Posts about building software."),
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
        "rust", "dioxus", "web", "sqlite", "ux", "performance", "async", "tooling", "career",
        "open-source",
    ];
    let mut tag_ids = Vec::new();
    for name in tags {
        let id: i64 = sqlx::query_scalar("INSERT INTO tags (name, slug) VALUES (?, ?) RETURNING id")
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

    println!("[seed] done: 3 users, 4 categories, 10 tags, 20 posts, 30 comments, 5 subscribers");
    Ok(())
}

const SAMPLE_TITLES: &[&str] = &[
    "Getting Started",
    "Lessons Learned",
    "A Deep Dive",
    "Patterns That Scale",
    "Notes from the Field",
];
