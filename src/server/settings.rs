//! Site-wide settings. Currently just the theme accent hue, which drives the
//! Tailwind `--brand-hue` knob (see `tailwind.css`). Reads are public (every
//! page needs the hue to render); writes require the settings capability.

use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::auth_tokens::SETTINGS_WRITE;
#[cfg(feature = "server")]
use crate::server::{require_perm, sfe, DbExtension};

/// Default accent hue — matches the compiled-in `--brand-hue` in tailwind.css
/// (≈ sky blue), so an un-themed site and the stylesheet agree.
pub const DEFAULT_THEME_HUE: i64 = 235;

/// The site's accent hue (oklch hue angle, 0–360). Public — the App root reads
/// it on every page to inject the `--brand-hue` override.
#[get("/api/theme", db: DbExtension)]
pub async fn get_theme_hue() -> Result<i64> {
    let stored: Option<String> =
        sqlx::query_scalar("SELECT value FROM site_settings WHERE key = 'theme_hue'")
            .fetch_optional(&db.0)
            .await
            .map_err(sfe)?;
    let hue = stored
        .and_then(|s| s.trim().parse::<i64>().ok())
        .map(|h| h.rem_euclid(360))
        .unwrap_or(DEFAULT_THEME_HUE);
    Ok(hue)
}

/// Set the site's accent hue (admin only). Clamped to 0–360.
#[post("/api/theme/set", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn set_theme_hue(hue: i64) -> Result<()> {
    require_perm(&auth, SETTINGS_WRITE)?;
    let hue = hue.rem_euclid(360);
    sqlx::query(
        "INSERT INTO site_settings (key, value) VALUES ('theme_hue', ?)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
    )
    .bind(hue.to_string())
    .execute(&db.0)
    .await
    .map_err(sfe)?;
    Ok(())
}
