//! Global permission tokens used to gate blog capabilities. Per-post ownership
//! is handled separately via arium's resource-membership model; these tokens
//! gate *capabilities* (who may author at all, moderate, manage, etc.) and act
//! as the admin override for `require_resource_or_permission`.

/// May create posts and upload media (the "author" capability).
pub const POSTS_WRITE: &str = "posts:write";
/// Global override: edit/delete *any* post regardless of per-post role (admins).
pub const POSTS_WRITE_ANY: &str = "posts:write_any";
/// May upload to the media library.
pub const MEDIA_UPLOAD: &str = "media:upload";
/// May approve/reject/delete comments.
pub const COMMENTS_MODERATE: &str = "comments:moderate";
/// May manage users & roles (arium admin console).
pub const USERS_MANAGE: &str = "admin:users:read";
/// May edit site settings, categories, and tags.
pub const SETTINGS_WRITE: &str = "settings:write";
/// May view analytics.
pub const ANALYTICS_READ: &str = "analytics:read";

/// Capability tokens that unlock the admin area / nav: holding ANY one shows the
/// Admin link and satisfies the admin-section route policy. Single source of
/// truth for the header gate (`layouts::SiteHeader`) and the page policy
/// (`pages::admin::admin_any_policy`). `MEDIA_UPLOAD` is intentionally excluded —
/// a plain author holds it but isn't an admin, and `POSTS_WRITE` already lets
/// them into the authoring screens.
pub const ADMIN_NAV_TOKENS: [&str; 6] = [
    POSTS_WRITE,
    POSTS_WRITE_ANY,
    COMMENTS_MODERATE,
    USERS_MANAGE,
    SETTINGS_WRITE,
    ANALYTICS_READ,
];

/// Every grantable capability token — the full set handed to a seeded admin.
/// Kept here next to [`ADMIN_NAV_TOKENS`] so the two related lists don't drift.
pub const ALL_TOKENS: [&str; 7] = [
    POSTS_WRITE,
    POSTS_WRITE_ANY,
    MEDIA_UPLOAD,
    COMMENTS_MODERATE,
    USERS_MANAGE,
    SETTINGS_WRITE,
    ANALYTICS_READ,
];
