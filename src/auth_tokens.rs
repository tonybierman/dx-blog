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
