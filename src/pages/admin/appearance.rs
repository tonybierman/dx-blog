//! Visual settings: the accent theme (a `--brand-hue` picker with live preview)
//! and the public home-page layout selector.

use dioxus::prelude::*;
use dioxus_sdk_time::use_debounce;
use std::future::Future;
use std::time::Duration;

use crate::components::text::{Mb, PageTitle, SectionTitle};
use crate::model::HomeLayout;
use crate::server::settings::{
    get_home_layout, get_theme_hue, set_home_layout, set_theme_hue, DEFAULT_THEME_HUE,
};

use super::AdminShell;

/// Run a fire-and-forget save and report its outcome into a status signal:
/// `ok_msg` on success, the friendly server error on failure. Collapses the
/// `spawn { match fut.await { Ok => set(ok), Err => set(friendly) } }` block the
/// settings savers all repeated (the two `ThemeSelector` savers were identical).
fn save_with_status(
    fut: impl Future<Output = Result<()>> + 'static,
    mut status: Signal<String>,
    ok_msg: impl Into<String>,
) {
    let ok_msg = ok_msg.into();
    spawn(async move {
        match fut.await {
            Ok(()) => status.set(ok_msg),
            Err(e) => status.set(arium_dioxus::friendly_server_error(e)),
        }
    });
}

/// Visual settings: the accent theme and the public home-page layout.
#[component]
pub fn AdminAppearance() -> Element {
    rsx! {
        AdminShell { active: "appearance".to_string(),
            PageTitle { "Appearance" }
            ThemeSelector {}
            HomeLayoutSelector {}
        }
    }
}

/// Live preview: set the hue signal and override the CSS var on `<html>` so the
/// whole page recolors immediately, independent of any save round-trip. A free
/// fn (not a closure) so it can be reused across several event handlers — the
/// `Signal` is `Copy`, so passing it by value still writes the shared state.
fn preview_hue(mut hue: Signal<i64>, h: i64) {
    hue.set(h);
    let _ = document::eval(&format!(
        "document.documentElement.style.setProperty('--brand-hue', '{h}')"
    ));
}

/// Accent-hue picker. Drives the Tailwind `--brand-hue` knob: presets + a
/// 0–360 slider, applied live on the document for instant preview and persisted
/// site-wide via `set_theme_hue`. Swatches are rendered with raw oklch so the
/// admin sees the actual accent at each hue.
#[component]
fn ThemeSelector() -> Element {
    let saved = use_resource(get_theme_hue);
    let mut hue = use_signal(|| DEFAULT_THEME_HUE);
    // `msg` is `Copy` and written only inside `save_with_status`, so it needn't be `mut` here.
    let msg = use_signal(String::new);

    // Persist the hue through a debounce so a flurry of preset clicks / slider
    // releases collapses to a single save of the final value — overlapping
    // `set_theme_hue` spawns could otherwise land out of order and persist a
    // stale hue. Live preview (preview_hue) stays instant; only the save waits.
    let mut save_hue = use_debounce(Duration::from_millis(300), move |h: i64| {
        save_with_status(set_theme_hue(h), msg, "Saved.");
    });

    // Sync the control to the stored hue once it loads.
    use_effect(move || {
        if let Some(Ok(h)) = &*saved.read() {
            hue.set(*h);
        }
    });

    let presets = [
        ("Sky", 235),
        ("Indigo", 265),
        ("Violet", 295),
        ("Pink", 330),
        ("Crimson", 15),
        ("Orange", 55),
        ("Emerald", 155),
        ("Teal", 195),
    ];

    rsx! {
        section { class: "mb-8 max-w-xl",
            SectionTitle { mb: Mb::Mb1, "Theme" }
            p { class: "mb-3 text-sm text-white/50",
                "Pick the site accent. Changes preview instantly and save site-wide."
            }
            div { class: "flex flex-wrap gap-2",
                for (name, h) in presets {
                    button {
                        key: "{name}",
                        r#type: "button",
                        class: if hue() == h {
                            "flex items-center gap-1.5 rounded-lg border border-white/50 px-3 py-1 text-xs"
                        } else {
                            "flex items-center gap-1.5 rounded-lg border border-white/15 px-3 py-1 text-xs hover:border-white/40"
                        },
                        onclick: move |_| {
                            preview_hue(hue, h);
                            save_hue.action(h);
                        },
                        span {
                            class: "inline-block h-3 w-3 rounded-full",
                            style: "background: oklch(68.5% 0.165 {h})",
                        }
                        "{name}"
                    }
                }
            }
            div { class: "mt-4 flex items-center gap-3",
                input {
                    r#type: "range",
                    min: "0",
                    max: "360",
                    value: "{hue}",
                    class: "w-64 accent-brand-500",
                    // Drag = live preview only (no save spam)…
                    oninput: move |e| { if let Ok(h) = e.value().parse::<i64>() { preview_hue(hue, h); } },
                    // …release = persist.
                    onchange: move |e| {
                        if let Ok(h) = e.value().parse::<i64>() {
                            preview_hue(hue, h);
                            save_hue.action(h);
                        }
                    },
                }
                span { class: "w-14 tabular-nums text-sm text-white/60", "{hue()}°" }
                span {
                    class: "inline-block h-7 w-7 rounded-full border border-white/20",
                    style: "background: oklch(58.8% 0.155 {hue})",
                }
                if !msg().is_empty() {
                    span { class: "text-xs text-white/50", "{msg}" }
                }
            }
        }
    }
}

/// Home-layout picker. Lets an admin choose which structural shell the public
/// home page renders the feed in (see [`HomeLayout`]). Each option shows a small
/// CSS sketch of the layout; clicking persists it site-wide via `set_home_layout`.
#[component]
fn HomeLayoutSelector() -> Element {
    let saved = use_resource(get_home_layout);
    let mut current = use_signal(HomeLayout::default);
    let msg = use_signal(String::new);

    // Sync the control to the stored layout once it loads.
    use_effect(move || {
        if let Some(Ok(l)) = &*saved.read() {
            current.set(*l);
        }
    });

    rsx! {
        section { class: "mb-8",
            SectionTitle { mb: Mb::Mb1, "Home layout" }
            p { class: "mb-3 text-sm text-white/50",
                "The structural shell the public home page renders the post feed in. Saves immediately."
            }
            div { class: "grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4",
                for layout in HomeLayout::ALL {
                    button {
                        key: "{layout.as_key()}",
                        r#type: "button",
                        class: if current() == layout {
                            "flex flex-col items-center gap-2 rounded-lg border border-brand-400 bg-white/[0.04] p-3 text-center"
                        } else {
                            "flex flex-col items-center gap-2 rounded-lg border border-white/10 p-3 text-center hover:border-white/40"
                        },
                        onclick: move |_| {
                            current.set(layout);
                            save_with_status(set_home_layout(layout), msg, format!("Saved “{}”.", layout.label()));
                        },
                        {layout_thumb(layout)}
                        span { class: "text-xs font-medium", "{layout.label()}" }
                        span { class: "text-[10px] leading-tight text-white/40", "{layout.blurb()}" }
                    }
                }
            }
            if !msg().is_empty() {
                p { class: "mt-3 text-xs text-white/50", "{msg}" }
            }
        }
    }
}

/// A ~96×64px CSS sketch of a layout's shape for the selector. `bar` blocks are
/// chrome, `cell` is the feed/content, `muted` is secondary regions.
fn layout_thumb(layout: HomeLayout) -> Element {
    const BAR: &str = "bg-white/25";
    const CELL: &str = "bg-brand-500/50";
    const MUTED: &str = "bg-white/10";
    const FRAME: &str =
        "flex h-16 w-24 flex-col gap-0.5 overflow-hidden rounded-lg border border-white/10 bg-black/30 p-1";

    match layout {
        HomeLayout::HolyGrail => rsx! {
            div { class: "{FRAME}",
                div { class: "h-1.5 {BAR}" }
                div { class: "flex flex-1 gap-0.5",
                    div { class: "w-2 {MUTED}" }
                    div { class: "flex-1 {CELL}" }
                    div { class: "w-2 {MUTED}" }
                }
                div { class: "h-1.5 {BAR}" }
            }
        },
        HomeLayout::StickySidebar => rsx! {
            div { class: "{FRAME}",
                div { class: "h-1.5 {BAR}" }
                div { class: "flex flex-1 gap-0.5",
                    div { class: "w-3 {MUTED}" }
                    div { class: "flex-1 {CELL}" }
                }
            }
        },
        HomeLayout::SplitScreen => rsx! {
            div { class: "{FRAME}",
                div { class: "flex flex-1 gap-0.5",
                    div { class: "flex-1 bg-white/30" }
                    div { class: "flex-1 {CELL}" }
                }
            }
        },
        HomeLayout::FullBleed => rsx! {
            div { class: "{FRAME}",
                div { class: "flex-1 {CELL}" }
            }
        },
        HomeLayout::Drawer => rsx! {
            div { class: "relative {FRAME}",
                div { class: "h-1.5 {BAR}" }
                div { class: "flex-1 {CELL}" }
                div { class: "absolute inset-y-1 left-1 w-3 rounded-lg bg-white/40" }
            }
        },
        HomeLayout::MegaMenu => rsx! {
            div { class: "{FRAME}",
                div { class: "h-1.5 {BAR}" }
                div { class: "h-3 bg-white/15" }
                div { class: "flex-1 {CELL}" }
            }
        },
        HomeLayout::BentoGrid => rsx! {
            div { class: "{FRAME}",
                div { class: "grid flex-1 grid-cols-3 grid-rows-2 gap-0.5",
                    div { class: "col-span-2 row-span-2 {CELL}" }
                    div { class: "{MUTED}" }
                    div { class: "{MUTED}" }
                }
            }
        },
        HomeLayout::Masonry => rsx! {
            div { class: "{FRAME}",
                div { class: "flex flex-1 gap-0.5",
                    div { class: "flex flex-1 flex-col gap-0.5",
                        div { class: "h-4 {CELL}" }
                        div { class: "flex-1 {MUTED}" }
                    }
                    div { class: "flex flex-1 flex-col gap-0.5",
                        div { class: "h-6 {MUTED}" }
                        div { class: "flex-1 {CELL}" }
                    }
                    div { class: "flex flex-1 flex-col gap-0.5",
                        div { class: "h-3 {CELL}" }
                        div { class: "flex-1 {MUTED}" }
                    }
                }
            }
        },
        HomeLayout::CardGrid => rsx! {
            div { class: "{FRAME}",
                div { class: "grid flex-1 grid-cols-3 grid-rows-2 gap-0.5",
                    for _ in 0..6 {
                        div { class: "{CELL}" }
                    }
                }
            }
        },
        HomeLayout::Editorial => rsx! {
            div { class: "{FRAME} items-center",
                div { class: "h-1.5 w-12 {BAR}" }
                div { class: "mt-0.5 flex w-full flex-1 justify-center gap-0.5",
                    div { class: "w-10 {CELL}" }
                    div { class: "w-3 {MUTED}" }
                }
            }
        },
        HomeLayout::HeroScroll => rsx! {
            div { class: "{FRAME}",
                div { class: "h-8 bg-white/30" }
                div { class: "flex-1 {CELL}" }
            }
        },
        HomeLayout::ScrollSticky => rsx! {
            div { class: "{FRAME}",
                div { class: "flex flex-1 gap-0.5",
                    div { class: "flex flex-1 flex-col gap-0.5",
                        div { class: "h-2 {CELL}" }
                        div { class: "h-2 {CELL}" }
                        div { class: "h-2 {CELL}" }
                    }
                    div { class: "w-8 {MUTED}" }
                }
            }
        },
    }
}
