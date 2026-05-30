//! Core site settings: the display title and tagline shown in the chrome. Theme
//! and home layout live under Appearance; categories and tags under Taxonomy.

use dioxus::prelude::*;

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::input::Input;
use crate::server::settings::{get_site_tagline, get_site_title, set_site_tagline, set_site_title};

use super::AdminShell;

/// Core site settings: the display title and tagline shown in the chrome. Theme
/// and home layout live under Appearance; categories and tags under Taxonomy.
#[component]
pub fn AdminSettings() -> Element {
    let title = use_resource(get_site_title);
    let tagline = use_resource(get_site_tagline);
    let mut title_draft = use_signal(String::new);
    let mut tagline_draft = use_signal(String::new);
    let mut saved = use_signal(|| false);
    let mut err = use_signal(String::new);

    // Seed the drafts from the stored values once they load.
    use_effect(move || {
        if let Some(Ok(t)) = &*title.read() {
            title_draft.set(t.clone());
        }
    });
    use_effect(move || {
        if let Some(Ok(t)) = &*tagline.read() {
            tagline_draft.set(t.clone());
        }
    });

    let save = move |_| {
        let t = title_draft();
        let g = tagline_draft();
        spawn(async move {
            // Run both saves unconditionally — `&&` would short-circuit, skipping
            // the tagline save whenever the title one failed. Then report exactly
            // which field(s) didn't persist instead of a blanket "not saved".
            let title_ok = set_site_title(t).await.is_ok();
            let tagline_ok = set_site_tagline(g).await.is_ok();
            if title_ok && tagline_ok {
                saved.set(true);
                err.set(String::new());
            } else {
                saved.set(false);
                let mut failed = Vec::new();
                if !title_ok {
                    failed.push("title");
                }
                if !tagline_ok {
                    failed.push("tagline");
                }
                err.set(format!("Couldn't save {}.", failed.join(" and ")));
            }
        });
    };

    rsx! {
        AdminShell { active: "settings".to_string(),
            h1 { class: "mb-6 text-2xl font-bold", "Site settings" }
            section { class: "max-w-xl space-y-4",
                div {
                    label { class: "mb-1 block text-sm font-medium", "Site title" }
                    Input {
                        class: "w-full",
                        placeholder: "dx-blog",
                        value: "{title_draft}",
                        oninput: move |e: FormEvent| { title_draft.set(e.value()); saved.set(false); err.set(String::new()); },
                    }
                    p { class: "mt-1 text-xs text-white/40", "Shown as the brand in the header and footer." }
                }
                div {
                    label { class: "mb-1 block text-sm font-medium", "Site tagline" }
                    Input {
                        class: "w-full",
                        placeholder: "A short subtitle for your site",
                        value: "{tagline_draft}",
                        oninput: move |e: FormEvent| { tagline_draft.set(e.value()); saved.set(false); err.set(String::new()); },
                    }
                    p { class: "mt-1 text-xs text-white/40", "Shown beside the title in the header." }
                }
                div { class: "flex items-center gap-3",
                    Button {
                        variant: ButtonVariant::Primary,
                        size: ButtonSize::Sm,
                        onclick: save,
                        "Save"
                    }
                    if saved() {
                        span { class: "text-sm text-green-400", "Saved" }
                    }
                    if !err().is_empty() {
                        span { class: "text-sm text-red-400", "{err}" }
                    }
                }
            }
        }
    }
}
