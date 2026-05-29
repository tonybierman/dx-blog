//! Reusable structural layout wrappers, distilled from the dioxus-mcp registry's
//! `holy_grail` / `full_bleed` / `bento_grid` / `masonry` skeletons into
//! children/slot-accepting components. Tailwind utility classes; responsive via
//! intrinsic sizing (`minmax`/`auto-fit`/`clamp`) — no hardcoded breakpoints
//! beyond the registry's `md:` switch points.

use dioxus::prelude::*;

use arium_dioxus::server::logout;
use arium_dioxus::ui::use_permissions;

use crate::auth_tokens::{
    ANALYTICS_READ, COMMENTS_MODERATE, POSTS_WRITE, POSTS_WRITE_ANY, SETTINGS_WRITE, USERS_MANAGE,
};
use crate::Route;

/// Persistent top navigation shared by chrome-bearing layouts. Reflects the
/// current sign-in state via arium's permissions context.
#[component]
pub fn SiteHeader() -> Element {
    let perms = use_permissions();
    let authed = perms.is_authenticated();
    let can_admin = authed
        && [
            POSTS_WRITE,
            POSTS_WRITE_ANY,
            COMMENTS_MODERATE,
            USERS_MANAGE,
            SETTINGS_WRITE,
            ANALYTICS_READ,
        ]
        .iter()
        .copied()
        .any(|t| perms.has(t));
    let name = perms
        .profile()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    rsx! {
        header { class: "w-full border-b border-white/10 bg-black/20 backdrop-blur",
            nav { class: "mx-auto flex max-w-6xl items-center justify-between gap-4 px-4 py-3",
                Link { to: Route::HomePage, class: "text-lg font-semibold tracking-tight", "dx-blog" }
                div { class: "flex items-center gap-4 text-sm",
                    Link { to: Route::HomePage, class: "hover:underline", "Home" }
                    Link { to: Route::Archive, class: "hover:underline", "Archive" }
                    Link { to: Route::SearchResults { q: String::new() }, class: "hover:underline", "Search" }
                    if authed {
                        if can_admin {
                            Link { to: Route::AdminDashboard, class: "hover:underline", "Admin" }
                        }
                        Link { to: Route::AccountPage, class: "hover:underline", "{name}" }
                        button {
                            class: "rounded border border-white/15 px-2 py-1 hover:bg-white/5",
                            onclick: move |_| {
                                spawn(async move {
                                    let _ = logout().await;
                                    perms.refresh();
                                    navigator().push(Route::HomePage);
                                });
                            },
                            "Sign out"
                        }
                    } else {
                        Link { to: Route::LoginPage, class: "hover:underline", "Sign in" }
                    }
                }
            }
        }
    }
}

/// Shared footer.
#[component]
pub fn SiteFooter() -> Element {
    rsx! {
        footer { class: "w-full border-t border-white/10 px-4 py-6 text-center text-sm text-white/50",
            "© dx-blog"
        }
    }
}

/// Holy-grail: persistent header, optional left/right sidebars flanking the main
/// column at `md`, and a footer. Sidebars collapse out on phones.
#[component]
pub fn HolyGrailLayout(
    #[props(optional)] left: Option<Element>,
    #[props(optional)] right: Option<Element>,
    children: Element,
) -> Element {
    rsx! {
        div { class: "flex min-h-screen flex-col",
            SiteHeader {}
            div { class: "mx-auto grid w-full max-w-6xl flex-1 gap-6 px-4 py-6 md:grid-cols-[200px_1fr_240px]",
                if let Some(left) = left {
                    aside { class: "hidden md:block", {left} }
                } else {
                    aside { class: "hidden md:block" }
                }
                main { class: "min-w-0", {children} }
                if let Some(right) = right {
                    aside { class: "hidden md:block", {right} }
                } else {
                    aside { class: "hidden md:block" }
                }
            }
            SiteFooter {}
        }
    }
}

/// Full-bleed: content fills the viewport with no persistent chrome.
#[component]
pub fn FullBleedLayout(children: Element) -> Element {
    rsx! {
        div { class: "min-h-screen w-full", {children} }
    }
}

/// Bento grid: an asymmetric tile grid. Callers pass `.bento-tile` children;
/// add `bento-feature` (2x2) / `bento-wide` (span 2) classes on tiles to vary.
#[component]
pub fn BentoGridLayout(#[props(optional)] left: Option<Element>, children: Element) -> Element {
    rsx! {
        div { class: "flex min-h-screen flex-col",
            SiteHeader {}
            main { class: "mx-auto w-full max-w-6xl flex-1 px-4 py-6",
                if let Some(left) = left {
                    div { class: "mb-4", {left} }
                }
                div { class: "grid auto-rows-[150px] grid-cols-2 gap-4 md:grid-cols-4", {children} }
            }
            SiteFooter {}
        }
    }
}

/// Masonry: staggered multi-column layout via CSS columns; items keep their
/// natural height and never break across columns.
#[component]
pub fn MasonryLayout(children: Element) -> Element {
    rsx! {
        div { class: "flex min-h-screen flex-col",
            SiteHeader {}
            main { class: "mx-auto w-full max-w-6xl flex-1 px-4 py-6",
                div { class: "gap-4 [column-gap:1rem] columns-1 sm:columns-2 lg:columns-3", {children} }
            }
            SiteFooter {}
        }
    }
}
