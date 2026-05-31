//! Authoring & admin mutations. Capability gating via global permission tokens;
//! per-post edit/delete via arium's resource-or-permission check (Editor on the
//! post OR the global `posts:write_any` admin token).
//!
//! Split by domain: [`posts`], [`comments`], [`taxonomy`], [`media`]. The shared
//! per-post authorization helper [`can_edit_post`] lives here; every server fn is
//! re-exported so `crate::server::admin::*` resolves it regardless of which
//! submodule defines it.

pub mod comments;
pub mod media;
pub mod posts;
pub mod taxonomy;

pub use comments::*;
pub use media::*;
pub use posts::*;
pub use taxonomy::*;

#[cfg(feature = "server")]
use crate::auth_tokens::POSTS_WRITE_ANY;
#[cfg(feature = "server")]
use dioxus::prelude::ServerFnError;

/// Edit/delete authorization: Editor+ on the post, OR a global admin token.
/// Shared by [`posts`] (update / edit-form read) and the public post page's
/// edit affordance ([`crate::server::posts`]).
#[cfg(feature = "server")]
pub(crate) async fn can_edit_post(
    auth: &arium_dioxus::auth::Session,
    db: &arium_dioxus::pool::Pool,
    authority: &arium_dioxus::ResourceAuthorityExt,
    post_id: i64,
) -> std::result::Result<i64, ServerFnError> {
    let uid = auth
        .current_user
        .as_ref()
        .filter(|u| !u.anonymous)
        .map(|u| u.id)
        .ok_or_else(|| ServerFnError::new("Not signed in."))?;
    arium_dioxus::require_resource_or_permission(
        authority.0.as_ref(),
        db,
        uid,
        arium_dioxus::ResourceRef::new("post", post_id),
        arium_dioxus::ResourceRole::Editor,
        POSTS_WRITE_ANY,
    )
    .await
    .map_err(|_| ServerFnError::new("You can't edit this post."))?;
    Ok(uid)
}
