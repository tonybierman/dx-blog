//! dx-blog — a full-featured blog on Dioxus Fullstack, with auth/authz from the
//! local `arium` workspace.
//!
//! - arium owns users / roles / sessions / password+OAuth flows and ships the
//!   drop-in auth UI. We reuse those.
//! - Per-post ownership uses arium's resource-membership model: `SqlMembershipStore`
//!   is registered as the `ResourceAuthority`, post creators are granted `Owner`,
//!   and mutations enforce `require_resource_or_permission` (Editor on the post OR
//!   a global admin token).
//! - The blog's own data (posts, comments, …) lives in `migrations/` and is
//!   reached from server fns through the shared `axum::Extension<Pool>`.

use dioxus::prelude::*;

use arium_dioxus::ui::components::input::Input;
use arium_dioxus::ui::components::label::Label;
use arium_dioxus::ui::{OAuthProvidersProvider, PermissionsProvider};

mod auth_tokens;
mod layouts;
mod model;
mod pages;
#[cfg(feature = "server")]
mod seed;
mod server;

use pages::admin::{
    AdminAnalytics, AdminAppearance, AdminComments, AdminDashboard, AdminMedia, AdminPostEdit,
    AdminPostList, AdminPostNew, AdminSettings, AdminTaxonomy, AdminUsers,
};
use pages::auth::{
    AccountPage, ForgotPasswordPage, LoginPage, RegisterPage, ResetPasswordPage, VerifyEmailPage,
};
use pages::errors::{NotFound, ServerError};
use pages::home::HomePage;
use pages::reader::{
    Archive, AuthorProfile, CategoryFeed, ConfirmSubscription, PostDetail, SearchResults,
    Subscribe, TagFeed,
};

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

/// The blog's routes. Each page component wraps its own body in the appropriate
/// layout wrapper (HolyGrail / FullBleed / Bento / Masonry) — see `layouts`.
#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    // --- Auth (FullBleed; arium UI) ---
    #[route("/login")]
    LoginPage,
    #[route("/register")]
    RegisterPage,
    #[route("/forgot-password")]
    ForgotPasswordPage,
    #[route("/auth/reset?:token")]
    ResetPasswordPage { token: String },
    #[route("/auth/verify?:token")]
    VerifyEmailPage { token: String },
    #[route("/account")]
    AccountPage,

    // --- Public / Reader ---
    #[route("/")]
    HomePage,
    #[route("/post/:slug")]
    PostDetail { slug: String },
    #[route("/category/:slug")]
    CategoryFeed { slug: String },
    #[route("/tag/:slug")]
    TagFeed { slug: String },
    #[route("/author/:slug")]
    AuthorProfile { slug: String },
    #[route("/archive")]
    Archive,
    #[route("/search?:q")]
    SearchResults { q: String },
    #[route("/subscribe")]
    Subscribe,
    #[route("/subscribe/confirm?:token")]
    ConfirmSubscription { token: String },

    // --- Admin (gated by RequirePermission in each page) ---
    #[route("/admin")]
    AdminDashboard,
    #[route("/admin/posts")]
    AdminPostList,
    #[route("/admin/posts/new")]
    AdminPostNew,
    #[route("/admin/posts/:id/edit")]
    AdminPostEdit { id: i64 },
    #[route("/admin/media")]
    AdminMedia,
    #[route("/admin/comments")]
    AdminComments,
    #[route("/admin/users")]
    AdminUsers,
    #[route("/admin/settings")]
    AdminSettings,
    #[route("/admin/appearance")]
    AdminAppearance,
    #[route("/admin/taxonomy")]
    AdminTaxonomy,
    #[route("/admin/analytics")]
    AdminAnalytics,

    // --- Fallbacks ---
    #[route("/500")]
    ServerError,
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

fn main() {
    #[cfg(not(feature = "server"))]
    dioxus::launch(App);

    #[cfg(feature = "server")]
    dioxus::serve(|| async {
        use std::sync::Arc;

        // Dev SQLite DB under ./data unless DATABASE_URL is set.
        let pool = {
            use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
            use std::str::FromStr;

            // SQLite creates the file but not its parent dir.
            std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/data")).ok();

            let connect_opts = match std::env::var("DATABASE_URL") {
                Ok(url) if !url.trim().is_empty() => SqliteConnectOptions::from_str(&url)?,
                _ => SqliteConnectOptions::new()
                    .filename(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blog.db"))
                    .create_if_missing(true),
            };
            SqlitePoolOptions::new()
                .max_connections(20)
                .connect_with(connect_opts)
                .await?
        };

        // arium owns its schema; membership_migrator adds arium_resource_members
        // (per-resource roles); then our own blog tables.
        arium_dioxus::migrator().run(&pool).await?;
        arium_dioxus::membership_migrator().run(&pool).await?;
        // Our blog schema runs as idempotent raw DDL rather than a tracked
        // sqlx migrator, so it doesn't share arium's `_sqlx_migrations` table
        // (which would otherwise flag arium's versions as "missing").
        sqlx::raw_sql(include_str!("../migrations/0001_blog.sql"))
            .execute(&pool)
            .await?;

        // Auto-seed demo data on a fresh database (no-op once posts exist).
        if let Err(e) = seed::run_if_empty(&pool).await {
            eprintln!("[seed] WARN: {e}");
        }

        let mailer = arium_dioxus::Mailer::from_env()?;
        println!("[startup] mailer backend: {}", mailer.describe());

        // SqlMembershipStore is arium's bundled ResourceAuthority over
        // arium_resource_members — register it so per-post role checks resolve.
        let authority: arium_dioxus::SharedResourceAuthority =
            Arc::new(arium_dioxus::SqlMembershipStore);

        let builder = arium_dioxus::AuthConfig::builder(pool, mailer).resource_authority(authority);
        // arium's default rate limit (burst 30, 1 req/s per IP) is far too tight
        // here: the limiter fronts *every* request, so one page load — dozens of
        // per-component CSS assets + the wasm bundle + a burst of feed/sidebar
        // server fns, all from a single dev IP — drains the burst and then 429s
        // the page's own data calls. Relax it; set DX_RATE_LIMIT=off to disable.
        let builder = {
            let rl = match std::env::var("DX_RATE_LIMIT").ok().as_deref() {
                Some("off") => None,
                _ => Some(arium_dioxus::RateLimitConfig {
                    burst: 4096,
                    per_second: 256,
                }),
            };
            builder.rate_limit(rl)
        };
        let builder = match arium_dioxus::oauth::github::GithubProvider::from_env()? {
            Some(gh) => {
                println!("[startup] GitHub OAuth: enabled");
                builder.oauth_provider(gh)?
            }
            None => {
                println!("[startup] GitHub OAuth: disabled (set GITHUB_CLIENT_ID + GITHUB_CLIENT_SECRET)");
                builder
            }
        };
        let cfg = builder.build()?;

        // Serve uploaded media statically from ./uploads.
        std::fs::create_dir_all("uploads").ok();
        let router = dioxus::server::router(App)
            .nest_service("/uploads", tower_http::services::ServeDir::new("uploads"))
            // Public XML endpoints. The shared pool reaches their handlers via
            // the `Extension<Pool>` that `install()` layers over the whole router.
            .route(
                "/sitemap.xml",
                axum::routing::get(server::feeds::sitemap_handler),
            )
            .route("/feed.xml", axum::routing::get(server::feeds::atom_handler));

        // Permanent redirects from the names people (and feed readers) commonly
        // guess to the canonical endpoints above — otherwise they fall through to
        // the SPA catch-all and render the client 404 page. One canonical URL each.
        let router = {
            use axum::{response::Redirect, routing::get};
            router
                .route(
                    "/rss.xml",
                    get(|| async { Redirect::permanent("/feed.xml") }),
                )
                .route("/rss", get(|| async { Redirect::permanent("/feed.xml") }))
                .route("/feed", get(|| async { Redirect::permanent("/feed.xml") }))
                .route(
                    "/atom.xml",
                    get(|| async { Redirect::permanent("/feed.xml") }),
                )
                .route(
                    "/site.xml",
                    get(|| async { Redirect::permanent("/sitemap.xml") }),
                )
        };

        arium_dioxus::install(router, cfg).await
    });
}

#[component]
fn App() -> Element {
    // Site accent: fetch the stored hue and override the `--brand-hue` knob
    // baked into tailwind.css. Until it resolves, the compiled-in default
    // applies, so a default-themed site shows no flash.
    let theme_hue = use_resource(crate::server::settings::get_theme_hue);

    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        // Site-wide Open Graph / Twitter card tags. Per-page tags (og:title,
        // og:description, og:image, og:url) are added by the page components.
        GlobalMeta {}
        // Atom feed autodiscovery — points readers/browsers at /feed.xml.
        document::Link {
            rel: "alternate",
            r#type: "application/atom+xml",
            title: "dx-blog feed",
            href: "/feed.xml",
        }
        // arium's catalog theme tokens (canonical — no vendored copy).
        document::Stylesheet { href: arium_dioxus::DEFAULT_THEME_CSS }
        document::Stylesheet { href: MAIN_CSS }
        document::Stylesheet { href: TAILWIND_CSS }

        // Runtime theme override. Loaded after the stylesheets so it wins the
        // cascade; recolors every brand-* utility site-wide.
        if let Some(Ok(hue)) = &*theme_hue.read() {
            style { {format!(":root {{ --brand-hue: {hue}; }}")} }
        }

        // Pre-mount catalog widgets so their css_module assets register on the
        // first render, avoiding an unstyled flash on the login/logout remount.
        div { style: "display: none", aria_hidden: "true",
            Input {}
            Label { html_for: "__preload" }
        }

        PermissionsProvider {
            OAuthProvidersProvider {
                // Catch any error thrown while rendering a route (e.g. a server
                // fn `?` that bubbled out of a component) and render the /500
                // page UI in place instead of leaving a blank screen.
                ErrorBoundary {
                    handle_error: |error: ErrorContext| {
                        let detail = error.error().map(|e| e.to_string()).unwrap_or_default();
                        rsx! { ServerError { detail } }
                    },
                    Router::<Route> {}
                }
            }
        }
    }
}

/// Site-wide `<head>` tags that don't vary per page: the Open Graph site name
/// (the configured site title) and the default Twitter card type. Resolved with
/// `use_server_future` so the tags are part of the server-rendered HTML where
/// crawlers and link-unfurlers — which don't run JavaScript — can read them.
/// Page components layer the per-page og:title / og:description / og:image /
/// og:url on top of these.
#[component]
fn GlobalMeta() -> Element {
    let meta = use_server_future(crate::server::settings::get_site_meta)?;
    let site_name = match &*meta.read() {
        Some(Ok(m)) => m.title.clone(),
        _ => crate::server::settings::DEFAULT_SITE_TITLE.to_string(),
    };
    rsx! {
        document::Meta { property: "og:site_name", content: "{site_name}" }
        document::Meta { name: "twitter:card", content: "summary_large_image" }
    }
}
