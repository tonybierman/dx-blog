//! Reusable structural layout wrappers, distilled from the dioxus-mcp registry's
//! layout skeletons into children/slot-accepting components. Covers the 12
//! home-page kinds the admin can select between (see [`crate::model::HomeLayout`]):
//! `holy_grail`, `sticky_sidebar`, `split_screen`, `full_bleed`, `drawer`,
//! `mega_menu`, `bento_grid`, `masonry`, `card_grid`, `editorial`, `hero_scroll`,
//! `scroll_sticky`. Tailwind utility classes; responsive via intrinsic sizing
//! (`minmax`/`auto-fit`/`clamp`) and the registry's `md:` (48rem) switch points.

use dioxus::prelude::*;

use arium_dioxus::server::logout;
use arium_dioxus::ui::use_permissions;

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::navbar::{Navbar, NavbarContent, NavbarItem, NavbarNav, NavbarTrigger};
use crate::model::SiteMeta;
use crate::server::settings::DEFAULT_SITE_TITLE;
use crate::Route;

/// Site chrome (title + tagline) resolved once per page and shared via context so
/// `SiteHeader` and `SiteFooter` don't each issue their own fetch. The App root
/// provides it (see `main::App`); both consumers fall back to the compiled-in
/// default until it resolves client-side.
pub type SiteChrome = Resource<Result<SiteMeta>>;

/// Persistent top navigation shared by chrome-bearing layouts. Reflects the
/// current sign-in state via arium's permissions context.
#[component]
pub fn SiteHeader() -> Element {
    let perms = use_permissions();
    let authed = perms.is_authenticated();
    // Send "Admin" to the first section this user can actually open, so editors
    // (no analytics access) aren't dropped on the Dashboard's permission error.
    let admin_route = authed
        .then(|| crate::pages::admin::admin_landing(|t| perms.has(t)))
        .flatten();
    let name = perms
        .profile()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    // Admin-configurable branding (see AdminSettings), shared via context with
    // SiteFooter. Falls back to the compiled-in default until the meta resolves.
    let chrome = use_context::<SiteChrome>();
    let (title, tagline) = match &*chrome.read() {
        Some(Ok(m)) => (
            if m.title.is_empty() {
                DEFAULT_SITE_TITLE.to_string()
            } else {
                m.title.clone()
            },
            (!m.tagline.is_empty()).then(|| m.tagline.clone()),
        ),
        _ => (DEFAULT_SITE_TITLE.to_string(), None),
    };

    rsx! {
        header { class: "w-full border-b border-white/10 bg-black/20 backdrop-blur",
            div { class: "mx-auto flex max-w-6xl items-center justify-between gap-4 px-4 py-3",
                Link { to: Route::HomePage, class: "flex items-baseline gap-2",
                    span { class: "text-lg font-semibold tracking-tight", "{title}" }
                    if let Some(tagline) = tagline {
                        span { class: "hidden text-sm text-white/40 sm:inline", "{tagline}" }
                    }
                }
                // Catalog menubar: flat top-level links plus a dropdown for the
                // signed-in account actions (keyboard-navigable, themed via the
                // dx-components tokens).
                Navbar { aria_label: "Primary",
                    NavbarItem { index: 0usize, value: "home".to_string(), to: Route::HomePage, "Home" }
                    NavbarItem { index: 1usize, value: "archive".to_string(), to: Route::Archive, "Archive" }
                    NavbarItem {
                        index: 2usize,
                        value: "search".to_string(),
                        to: Route::SearchResults { q: String::new() },
                        "Search"
                    }
                    if authed {
                        if let Some(route) = admin_route.clone() {
                            NavbarItem { index: 3usize, value: "admin".to_string(), to: route, "Admin" }
                        }
                        NavbarNav { index: 4usize,
                            NavbarTrigger { "{name}" }
                            NavbarContent {
                                NavbarItem {
                                    index: 0usize,
                                    value: "account".to_string(),
                                    to: Route::AccountPage,
                                    "Account"
                                }
                                NavbarItem {
                                    index: 1usize,
                                    value: "signout".to_string(),
                                    to: Route::HomePage,
                                    onclick: move |_| {
                                        spawn(async move {
                                            let _ = logout().await;
                                            perms.refresh();
                                        });
                                    },
                                    "Sign out"
                                }
                            }
                        }
                    } else {
                        NavbarItem {
                            index: 3usize,
                            value: "signin".to_string(),
                            to: Route::LoginPage,
                            "Sign in"
                        }
                    }
                }
            }
        }
    }
}

/// Shared footer.
#[component]
pub fn SiteFooter() -> Element {
    let chrome = use_context::<SiteChrome>();
    let title = match &*chrome.read() {
        Some(Ok(m)) if !m.title.is_empty() => m.title.clone(),
        _ => DEFAULT_SITE_TITLE.to_string(),
    };
    rsx! {
        footer { class: "w-full border-t border-white/10 px-4 py-6 text-center text-sm text-white/50",
            "© {title}"
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
    // Only reserve a sidebar track when that slot is actually filled, so a page
    // using one sidebar (e.g. search → right only) lets the main column claim
    // the freed space instead of leaving a dead gutter.
    let cols = match (left.is_some(), right.is_some()) {
        (true, true) => "md:grid-cols-[200px_1fr_240px]",
        (true, false) => "md:grid-cols-[200px_1fr]",
        (false, true) => "md:grid-cols-[1fr_240px]",
        (false, false) => "md:grid-cols-[1fr]",
    };
    rsx! {
        div { class: "flex min-h-screen flex-col",
            SiteHeader {}
            div {
                class: "mx-auto grid w-full max-w-6xl flex-1 gap-6 px-4 py-6 {cols}",
                if let Some(left) = left {
                    aside { class: "hidden md:block", {left} }
                }
                main { class: "min-w-0", {children} }
                if let Some(right) = right {
                    aside { class: "hidden md:block", {right} }
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
                div { class: "grid auto-rows-[minmax(180px,auto)] grid-cols-2 gap-4 md:grid-cols-4", {children} }
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

/// Sticky sidebar: a nav strip on phones that becomes a pinned full-height left
/// column at `md`, beside a scrolling main pane.
#[component]
pub fn StickySidebarLayout(nav: Element, children: Element) -> Element {
    rsx! {
        div { class: "flex min-h-screen flex-col",
            SiteHeader {}
            div { class: "mx-auto flex w-full max-w-6xl flex-1 flex-col md:flex-row",
                aside { class: "border-b border-white/10 p-4 md:h-screen md:w-56 md:shrink-0 md:self-start md:sticky md:top-0 md:overflow-y-auto md:border-b-0 md:border-r",
                    {nav}
                }
                main { class: "min-w-0 flex-1 p-6", {children} }
            }
            SiteFooter {}
        }
    }
}

/// Split screen: an inverted intro pane and the feed — stacked on phones, two
/// equal columns at `md`.
#[component]
pub fn SplitScreenLayout(intro: Element, children: Element) -> Element {
    rsx! {
        div { class: "flex min-h-screen flex-col",
            SiteHeader {}
            div { class: "grid flex-1 grid-cols-1 md:grid-cols-2",
                div { class: "flex items-center justify-center bg-black/40 p-12", {intro} }
                main { class: "min-w-0 p-8", {children} }
            }
            SiteFooter {}
        }
    }
}

/// Drawer: an off-canvas nav panel that slides in from the left over a scrim,
/// toggled by a hamburger button. The feed stays beneath.
#[component]
pub fn DrawerLayout(nav: Element, children: Element) -> Element {
    let mut open = use_signal(|| false);
    rsx! {
        div { class: "relative flex min-h-screen flex-col",
            SiteHeader {}
            div { class: "flex items-center gap-3 border-b border-white/10 px-4 py-2",
                Button {
                    variant: ButtonVariant::Outline,
                    size: ButtonSize::Icon,
                    onclick: move |_| open.set(!open()),
                    "\u{2630}"
                }
                span { class: "text-sm text-white/60", "Browse" }
            }
            if open() {
                div {
                    class: "fixed inset-0 z-30 bg-black/50",
                    onclick: move |_| open.set(false),
                }
            }
            aside {
                class: if open() {
                    "fixed inset-y-0 left-0 z-40 w-64 translate-x-0 border-r border-white/10 bg-[#111] p-4 transition-transform"
                } else {
                    "fixed inset-y-0 left-0 z-40 w-64 -translate-x-full border-r border-white/10 bg-[#111] p-4 transition-transform"
                },
                {nav}
            }
            main { class: "mx-auto w-full max-w-6xl flex-1 p-6", {children} }
            SiteFooter {}
        }
    }
}

/// Mega menu: a top trigger drops a full-width panel under the bar; the feed
/// sits below it.
#[component]
pub fn MegaMenuLayout(panel: Element, children: Element) -> Element {
    let mut open = use_signal(|| false);
    rsx! {
        div { class: "flex min-h-screen flex-col",
            SiteHeader {}
            header { class: "relative border-b border-white/10",
                nav { class: "mx-auto flex max-w-6xl items-center gap-6 px-4 py-3",
                    span { class: "font-semibold", "Explore" }
                    Button {
                        variant: ButtonVariant::Ghost,
                        size: ButtonSize::Sm,
                        onclick: move |_| open.set(!open()),
                        "Browse ▾"
                    }
                }
                if open() {
                    div { class: "absolute inset-x-0 top-full z-30 border-b border-white/10 bg-[#111] shadow-xl",
                        div { class: "mx-auto grid max-w-6xl gap-6 px-4 py-6 sm:grid-cols-2 md:grid-cols-4",
                            {panel}
                        }
                    }
                }
            }
            main { class: "mx-auto w-full max-w-6xl flex-1 p-6", {children} }
            SiteFooter {}
        }
    }
}

/// Card grid: a titled, centered container — children supply the responsive
/// card grid itself (e.g. `FeedGrid`).
#[component]
pub fn CardGridLayout(children: Element) -> Element {
    rsx! {
        div { class: "flex min-h-screen flex-col",
            SiteHeader {}
            main { class: "mx-auto w-full max-w-6xl flex-1 px-4 py-8", {children} }
            SiteFooter {}
        }
    }
}

/// Editorial: a centered reading measure that splits into an asymmetric copy +
/// aside at `md`.
#[component]
pub fn EditorialLayout(#[props(optional)] sidebar: Option<Element>, children: Element) -> Element {
    rsx! {
        div { class: "flex min-h-screen flex-col",
            SiteHeader {}
            main { class: "mx-auto w-full max-w-4xl flex-1 px-4 py-8",
                if let Some(sidebar) = sidebar {
                    div { class: "grid gap-8 md:grid-cols-[2fr_1fr]",
                        div { class: "min-w-0", {children} }
                        aside { class: "md:border-l md:border-white/10 md:pl-6", {sidebar} }
                    }
                } else {
                    div { {children} }
                }
            }
            SiteFooter {}
        }
    }
}

/// Hero scroll: a tall inverted opening section above the fold, then the feed in
/// a centered measure below.
#[component]
pub fn HeroScrollLayout(#[props(optional)] hero: Option<Element>, children: Element) -> Element {
    rsx! {
        div { class: "flex min-h-screen flex-col",
            SiteHeader {}
            section { class: "flex min-h-[70vh] flex-col items-center justify-center bg-black/40 px-6 py-20 text-center",
                if let Some(hero) = hero {
                    {hero}
                } else {
                    h1 { class: "text-4xl font-bold", "dx-blog" }
                    p { class: "mt-4 text-white/60", "Latest writing, below." }
                }
            }
            main { class: "mx-auto w-full max-w-5xl flex-1 px-4 py-10", {children} }
            SiteFooter {}
        }
    }
}

/// Scroll-sticky: scrolling content beside a panel that pins in place at `md`
/// (the visual is hidden on phones, where there's nothing to pin against).
#[component]
pub fn ScrollStickyLayout(visual: Element, children: Element) -> Element {
    rsx! {
        div { class: "flex min-h-screen flex-col",
            SiteHeader {}
            main { class: "mx-auto w-full max-w-6xl flex-1 px-4 py-8",
                div { class: "grid gap-8 md:grid-cols-[1fr_320px]",
                    div { class: "min-w-0", {children} }
                    aside { class: "hidden md:block",
                        div { class: "sticky top-6", {visual} }
                    }
                }
            }
            SiteFooter {}
        }
    }
}
