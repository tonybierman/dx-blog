//! Site-wide settings. Currently just the theme accent hue, which drives the
//! Tailwind `--brand-hue` knob (see `tailwind.css`). Reads are public (every
//! page needs the hue to render); writes require the settings capability.

use dioxus::prelude::*;

use crate::model::{HomeLayout, SiteMeta};

#[cfg(feature = "server")]
use crate::auth_tokens::SETTINGS_WRITE;
#[cfg(feature = "server")]
use crate::db::settings::{get_setting_db, set_setting_db};
#[cfg(feature = "server")]
use crate::server::{require_perm, sfe, DbExtension};

/// Default accent hue — matches the compiled-in `--brand-hue` in tailwind.css
/// (≈ sky blue), so an un-themed site and the stylesheet agree.
pub const DEFAULT_THEME_HUE: i64 = 235;

/// The site's accent hue (oklch hue angle, 0–359). Public — the App root reads
/// it on every page to inject the `--brand-hue` override.
#[get("/api/theme", db: DbExtension)]
pub async fn get_theme_hue() -> Result<i64> {
    let hue = get_setting_db(&db.0, "theme_hue")
        .await
        .map_err(sfe)?
        .and_then(|s| s.trim().parse::<i64>().ok())
        .map(|h| h.rem_euclid(360))
        .unwrap_or(DEFAULT_THEME_HUE);
    Ok(hue)
}

/// Set the site's accent hue (admin only). Normalised to 0–359 via `rem_euclid`.
#[post("/api/theme/set", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn set_theme_hue(hue: i64) -> Result<()> {
    require_perm(&auth, SETTINGS_WRITE)?;
    let hue = hue.rem_euclid(360);
    set_setting_db(&db.0, "theme_hue", &hue.to_string())
        .await
        .map_err(sfe)?;
    Ok(())
}

/// Default site title — the hard-coded brand used before an admin sets one.
pub const DEFAULT_SITE_TITLE: &str = "riparion-cms";

/// The site's display title (shown in the header/footer brand). Public — chrome
/// reads it on every page. Falls back to [`DEFAULT_SITE_TITLE`] when unset.
#[get("/api/site-title", db: DbExtension)]
pub async fn get_site_title() -> Result<String> {
    let title = get_setting_db(&db.0, "site_title")
        .await
        .map_err(sfe)?
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_SITE_TITLE.to_string());
    Ok(title)
}

/// Set the site title (admin only).
#[post("/api/site-title/set", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn set_site_title(title: String) -> Result<()> {
    require_perm(&auth, SETTINGS_WRITE)?;
    set_setting_db(&db.0, "site_title", title.trim())
        .await
        .map_err(sfe)?;
    Ok(())
}

/// The site's tagline (a short subtitle shown beside the brand). Public — chrome
/// reads it on every page. Empty string when unset.
#[get("/api/site-tagline", db: DbExtension)]
pub async fn get_site_tagline() -> Result<String> {
    let tagline = get_setting_db(&db.0, "site_tagline")
        .await
        .map_err(sfe)?
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    Ok(tagline)
}

/// Site-level metadata used to build the `<head>` / Open Graph tags: display
/// title (falls back to [`DEFAULT_SITE_TITLE`]), tagline, and the canonical
/// origin for absolute URLs (from `SITE_URL`). Public — every page's head reads
/// it. Bundled into one call so a page resolves all three in a single round trip.
#[get("/api/site-meta", db: DbExtension)]
pub async fn get_site_meta() -> Result<SiteMeta> {
    let title = get_setting_db(&db.0, "site_title").await.map_err(sfe)?;
    let tagline = get_setting_db(&db.0, "site_tagline").await.map_err(sfe)?;
    Ok(SiteMeta {
        title: title
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_SITE_TITLE.to_string()),
        tagline: tagline.map(|s| s.trim().to_string()).unwrap_or_default(),
        base_url: crate::server::feeds::site_base(),
    })
}

/// Set the site tagline (admin only).
#[post("/api/site-tagline/set", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn set_site_tagline(tagline: String) -> Result<()> {
    require_perm(&auth, SETTINGS_WRITE)?;
    set_setting_db(&db.0, "site_tagline", tagline.trim())
        .await
        .map_err(sfe)?;
    Ok(())
}

/// The layout the public home page renders the post feed in. Public — the home
/// page reads it on every load. Falls back to the default (Holy Grail) when
/// unset or when an unrecognized key is stored.
#[get("/api/home-layout", db: DbExtension)]
pub async fn get_home_layout() -> Result<HomeLayout> {
    let layout = get_setting_db(&db.0, "home_layout")
        .await
        .map_err(sfe)?
        .and_then(|s| HomeLayout::from_key(s.trim()))
        .unwrap_or_default();
    Ok(layout)
}

/// Set the home-page layout (admin only). Takes an argument, so this is a POST
/// (a GET request can't carry a body).
#[post("/api/home-layout/set", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn set_home_layout(layout: HomeLayout) -> Result<()> {
    require_perm(&auth, SETTINGS_WRITE)?;
    set_setting_db(&db.0, "home_layout", layout.as_key())
        .await
        .map_err(sfe)?;
    Ok(())
}
