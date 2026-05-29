//! Admin & authoring surface. The server fns are the real authorization
//! boundary; `RequirePermission` here just keeps unauthorized users out of the
//! UI and redirects them to sign in.
//!
//! Split by section: [`dashboard`] (dashboard + analytics), [`posts`] (list +
//! editor), [`comments`], [`media`], [`users`], [`settings`], [`appearance`]
//! (theme + home layout), [`taxonomy`]. The shared chrome ([`AdminShell`]) and
//! the fire-and-refetch [`ActionButton`] live here; each section's page
//! components are re-exported so `crate::pages::admin::*` resolves them.

use dioxus::prelude::*;
use std::future::Future;
use std::pin::Pin;

use arium_dioxus::ui::{Policy, RequirePermission};

use crate::auth_tokens::ADMIN_NAV_TOKENS;
use crate::Route;

mod appearance;
mod comments;
mod dashboard;
mod media;
mod posts;
mod settings;
mod taxonomy;
mod users;

pub use appearance::AdminAppearance;
pub use comments::AdminComments;
pub use dashboard::{AdminAnalytics, AdminDashboard};
pub use media::AdminMedia;
pub use posts::{AdminPostEdit, AdminPostList, AdminPostNew};
pub use settings::AdminSettings;
pub use taxonomy::AdminTaxonomy;
pub use users::AdminUsers;

fn admin_any_policy() -> Policy {
    Policy::any_of(ADMIN_NAV_TOKENS)
}

fn nav_class(active: &str, name: &str) -> &'static str {
    if active == name {
        "rounded bg-white/10 px-3 py-1.5 font-medium"
    } else {
        "rounded px-3 py-1.5 text-white/60 hover:bg-white/5 hover:text-white"
    }
}

/// Shared admin chrome: the sidebar nav (gated behind [`admin_any_policy`]) plus
/// a content slot. Every admin page wraps its body in this, passing the `active`
/// nav key so the current section is highlighted.
#[component]
pub(crate) fn AdminShell(active: String, children: Element) -> Element {
    rsx! {
        RequirePermission {
            policy: admin_any_policy(),
            redirect_to: "/login".to_string(),
            div { class: "flex min-h-screen",
                aside { class: "w-56 shrink-0 border-r border-white/10 bg-black/20 p-4",
                    h2 { class: "mb-4 text-lg font-bold", "Admin" }
                    nav { class: "flex flex-col gap-1 text-sm",
                        Link { to: Route::AdminDashboard, class: nav_class(&active, "dashboard"), "Dashboard" }
                        Link { to: Route::AdminPostList, class: nav_class(&active, "posts"), "Posts" }
                        Link { to: Route::AdminPostNew, class: nav_class(&active, "new"), "New post" }
                        Link { to: Route::AdminMedia, class: nav_class(&active, "media"), "Media" }
                        Link { to: Route::AdminComments, class: nav_class(&active, "comments"), "Comments" }
                        Link { to: Route::AdminUsers, class: nav_class(&active, "users"), "Users" }
                        Link { to: Route::AdminSettings, class: nav_class(&active, "settings"), "Settings" }
                        Link { to: Route::AdminAppearance, class: nav_class(&active, "appearance"), "Appearance" }
                        Link { to: Route::AdminTaxonomy, class: nav_class(&active, "taxonomy"), "Taxonomy" }
                        Link { to: Route::AdminAnalytics, class: nav_class(&active, "analytics"), "Analytics" }
                    }
                    Link { to: Route::HomePage, class: "mt-6 block text-xs text-white/40 hover:underline", "← Back to site" }
                }
                main { class: "flex-1 p-6", {children} }
            }
        }
    }
}

/// The future an [`ActionButton`]'s `action` resolves to. Boxed so a single prop
/// type can carry any server-fn call.
pub(crate) type ActionFuture = Pin<Box<dyn Future<Output = Result<()>>>>;

/// One button for a fire-and-refetch admin mutation. Collapses the repeated
/// `spawn → server_fn → refetch` boilerplate (and its silent error swallowing)
/// that lived in five near-identical button components and two inline closures:
/// it runs `action`, calls `on_done` on success, shows the error inline on
/// failure, and blocks a double-click while the request is in flight.
#[component]
pub(crate) fn ActionButton(
    label: String,
    #[props(default = "text-brand-400 hover:underline".to_string())] class: String,
    on_done: EventHandler<()>,
    action: Callback<(), ActionFuture>,
) -> Element {
    let mut busy = use_signal(|| false);
    let mut err = use_signal(String::new);
    rsx! {
        button {
            class: "{class} disabled:opacity-50",
            disabled: busy(),
            onclick: move |_| {
                if busy() {
                    return;
                }
                busy.set(true);
                err.set(String::new());
                spawn(async move {
                    match action.call(()).await {
                        Ok(()) => on_done.call(()),
                        Err(e) => err.set(arium_dioxus::friendly_server_error(e)),
                    }
                    busy.set(false);
                });
            },
            "{label}"
        }
        if !err().is_empty() {
            span { class: "ml-2 text-xs text-red-400", "{err}" }
        }
    }
}
