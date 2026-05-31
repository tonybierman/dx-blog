//! Taxonomy management: side-by-side category and tag editors, both driven by a
//! single parameterized [`TaxonomyEditor`] (the two differed only in which server
//! fn to call and their labels).

use dioxus::prelude::*;

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::input::Input;
use crate::components::text::{ErrorText, PageTitle, SectionTitle};
use crate::server::admin::{
    create_category, create_tag, delete_category, delete_tag, rename_category, rename_tag,
};
use crate::server::taxonomy::{list_categories, list_tags};

use super::AdminShell;

#[component]
pub fn AdminTaxonomy() -> Element {
    rsx! {
        AdminShell { active: "taxonomy".to_string(),
            PageTitle { "Taxonomy" }
            div { class: "grid gap-8 md:grid-cols-2",
                TaxonomyEditor { kind: TaxKind::Category }
                TaxonomyEditor { kind: TaxKind::Tag }
            }
        }
    }
}

/// Which taxonomy a [`TaxonomyEditor`] manages. The category and tag editors were
/// ~140 lines of near-identical add / list / inline-rename / delete markup; this
/// enum is the only thing that differed (which server fn to call, the labels), so
/// one parameterized component now serves both.
#[derive(Clone, Copy, PartialEq)]
enum TaxKind {
    Category,
    Tag,
}

impl TaxKind {
    fn title(self) -> &'static str {
        match self {
            TaxKind::Category => "Categories",
            TaxKind::Tag => "Tags",
        }
    }
    fn placeholder(self) -> &'static str {
        match self {
            TaxKind::Category => "New category",
            TaxKind::Tag => "New tag",
        }
    }
}

/// Add / list / inline-rename / delete editor for one taxonomy. Errors from any
/// server fn surface inline (previously they were silently swallowed by
/// `if …is_ok()`), and a rename to an empty name is rejected client-side to match
/// the server's own guard.
#[component]
fn TaxonomyEditor(kind: TaxKind) -> Element {
    // Both list fns return different row types; normalize to (id, name).
    let mut items = use_resource(move || async move {
        match kind {
            TaxKind::Category => list_categories()
                .await
                .map(|v| v.into_iter().map(|c| (c.id, c.name)).collect::<Vec<_>>()),
            TaxKind::Tag => list_tags()
                .await
                .map(|v| v.into_iter().map(|t| (t.id, t.name)).collect::<Vec<_>>()),
        }
    });
    let mut new_name = use_signal(String::new);
    // Inline-rename state: which row (by id) is being edited, and its draft name.
    let mut edit_id = use_signal::<Option<i64>>(|| None);
    let mut edit_name = use_signal(String::new);
    let mut err = use_signal(String::new);

    let add = move |_| {
        let name = new_name().trim().to_string();
        if name.is_empty() {
            return;
        }
        spawn(async move {
            let res = match kind {
                TaxKind::Category => create_category(name, None).await.map(|_| ()),
                TaxKind::Tag => create_tag(name).await.map(|_| ()),
            };
            match res {
                Ok(()) => {
                    new_name.set(String::new());
                    err.set(String::new());
                    items.restart();
                }
                Err(e) => err.set(arium_dioxus::friendly_server_error(e)),
            }
        });
    };

    rsx! {
        section {
            SectionTitle { "{kind.title()}" }
            div { class: "mb-3 flex gap-2",
                Input {
                    class: "flex-1",
                    placeholder: "{kind.placeholder()}",
                    value: "{new_name}",
                    oninput: move |e: FormEvent| new_name.set(e.value()),
                    onkeydown: move |e: KeyboardEvent| if e.key() == Key::Enter { add(()) },
                }
                Button { variant: ButtonVariant::Primary, size: ButtonSize::Sm, onclick: move |_| add(()), "Add" }
            }
            if !err().is_empty() {
                ErrorText { class: "mb-2 text-xs".to_string(), "{err}" }
            }
            match &*items.read() {
                Some(Ok(list)) if !list.is_empty() => {
                    let list = list.clone();
                    rsx! {
                        ul { class: "space-y-1 text-sm",
                            for (id, name) in list {
                                {
                                    let display = name.clone();
                                    let save = move |_| {
                                        spawn(async move {
                                            let new = edit_name().trim().to_string();
                                            // Match the server's non-empty guard, with feedback.
                                            if new.is_empty() {
                                                err.set("Name can't be empty.".into());
                                                return;
                                            }
                                            let res = match kind {
                                                TaxKind::Category => rename_category(id, new).await,
                                                TaxKind::Tag => rename_tag(id, new).await,
                                            };
                                            match res {
                                                Ok(()) => { edit_id.set(None); err.set(String::new()); items.restart(); }
                                                Err(e) => err.set(arium_dioxus::friendly_server_error(e)),
                                            }
                                        });
                                    };
                                    let del = move |_| {
                                        spawn(async move {
                                            let res = match kind {
                                                TaxKind::Category => delete_category(id).await,
                                                TaxKind::Tag => delete_tag(id).await,
                                            };
                                            match res {
                                                Ok(()) => items.restart(),
                                                Err(e) => err.set(arium_dioxus::friendly_server_error(e)),
                                            }
                                        });
                                    };
                                    rsx! {
                                        li { key: "{id}", class: "flex items-center justify-between gap-2",
                                            if edit_id() == Some(id) {
                                                Input {
                                                    class: "flex-1",
                                                    value: "{edit_name}",
                                                    oninput: move |e: FormEvent| edit_name.set(e.value()),
                                                    onkeydown: move |e: KeyboardEvent| if e.key() == Key::Enter { save(()) },
                                                }
                                                Button { variant: ButtonVariant::Link, size: ButtonSize::Xs, onclick: move |_| save(()), "Save" }
                                                Button { variant: ButtonVariant::Ghost, size: ButtonSize::Xs, onclick: move |_| edit_id.set(None), "Cancel" }
                                            } else {
                                                span { "{display}" }
                                                div { class: "flex gap-2",
                                                    Button {
                                                        variant: ButtonVariant::Ghost,
                                                        size: ButtonSize::Xs,
                                                        onclick: move |_| { edit_name.set(display.clone()); edit_id.set(Some(id)); },
                                                        "Edit"
                                                    }
                                                    Button { variant: ButtonVariant::Destructive, size: ButtonSize::Xs, onclick: del, "Delete" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Some(Ok(_)) => rsx! { p { class: "text-sm text-white/40", "None yet." } },
                Some(Err(e)) => rsx! { ErrorText { small: true, "{e}" } },
                None => rsx! { p { class: "text-sm text-white/50", "Loading…" } },
            }
        }
    }
}
