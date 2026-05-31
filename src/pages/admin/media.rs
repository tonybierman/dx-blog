//! Media library: upload an image (read as base64 in the browser, posted to the
//! server) and browse / copy-url / delete existing uploads.

use dioxus::prelude::*;

use crate::components::alert_dialog::{
    AlertDialog, AlertDialogAction, AlertDialogActions, AlertDialogCancel, AlertDialogDescription,
    AlertDialogTitle,
};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::surface::{Panel, PanelPadding, PanelVariant};
use crate::components::text::PageTitle;
use crate::pages::widgets::list_states;
use crate::server::admin::{delete_media, list_media, media_usage, upload_media};

use super::AdminShell;

#[component]
pub fn AdminMedia() -> Element {
    let mut media = use_resource(list_media);
    let mut msg = use_signal(String::new);

    let upload = move |_| {
        spawn(async move {
            let mut eval = document::eval(
                r#"
                const inp = document.getElementById('mediafile');
                const f = inp && inp.files && inp.files[0];
                if (!f) { dioxus.send(''); }
                else {
                    const r = new FileReader();
                    r.onload = () => { dioxus.send(f.name + '|' + r.result.split(',')[1]); };
                    // Without an onerror, a read failure leaves recv() awaiting a
                    // message that never arrives — the await would hang forever.
                    r.onerror = () => { dioxus.send('__read_error__'); };
                    r.readAsDataURL(f);
                }
                "#,
            );
            match eval.recv::<String>().await {
                Ok(s) if s == "__read_error__" => msg.set("Could not read file.".into()),
                Ok(s) if !s.is_empty() => {
                    if let Some((name, b64)) = s.split_once('|') {
                        match upload_media(name.to_string(), b64.to_string()).await {
                            Ok(_) => {
                                msg.set("Uploaded.".into());
                                media.restart();
                            }
                            Err(e) => msg.set(arium_dioxus::friendly_server_error(e)),
                        }
                    }
                }
                Ok(_) => msg.set("Choose a file first.".into()),
                Err(_) => msg.set("Could not read file.".into()),
            }
        });
    };

    rsx! {
        AdminShell { active: "media".to_string(),
            PageTitle { "Media library" }
            div { class: "mb-6 flex items-center gap-3",
                input { id: "mediafile", r#type: "file", accept: "image/*", class: "text-sm" }
                Button { variant: ButtonVariant::Primary, size: ButtonSize::Sm, onclick: upload, "Upload" }
                if !msg().is_empty() { span { class: "text-sm text-white/60", "{msg}" } }
            }
            {list_states!(media, empty: "No media yet.", list => rsx! {
                    div { class: "columns-2 gap-4 md:columns-3 lg:columns-4",
                        for m in list {
                            Panel { key: "{m.id}", variant: PanelVariant::Outlined, padding: PanelPadding::Sm, class: "mb-4 inline-block w-full break-inside-avoid".to_string(),
                                img { class: "w-full rounded-lg", src: "{m.url}", alt: "{m.filename}" }
                                div { class: "mt-1 flex items-center justify-between gap-2",
                                    button {
                                        class: "truncate text-left text-xs text-white/60 hover:text-white",
                                        title: "Copy URL",
                                        onclick: {
                                            let url = m.url.clone();
                                            move |_| {
                                                let url = url.clone();
                                                let _ = document::eval(&format!("navigator.clipboard.writeText('{url}')"));
                                            }
                                        },
                                        "{m.url}"
                                    }
                                    MediaDeleteButton {
                                        id: m.id,
                                        usage_count: m.usage_count,
                                        on_deleted: move |_| media.restart(),
                                    }
                                }
                                // Usage indicator — WordPress-style "where is this used".
                                div { class: "mt-1 text-[0.7rem]",
                                    if m.usage_count > 0 {
                                        span { class: "text-emerald-300/80", "Used in {m.usage_count} post(s)" }
                                    } else {
                                        span { class: "text-white/30", "Unused" }
                                    }
                                }
                            }
                        }
                    }
            })}
        }
    }
}

/// Delete control for one media item. Every delete now routes through an
/// [`AlertDialog`] confirmation (replacing the old `window.confirm`). When the
/// image is in use, opening the dialog lazily fetches the referencing posts and
/// lists them so the admin sees exactly what will break.
#[component]
fn MediaDeleteButton(id: i64, usage_count: i64, on_deleted: EventHandler<()>) -> Element {
    let mut open = use_signal(|| false);
    // The "used in these posts" list, fetched on open for in-use images only.
    let mut detail = use_signal(String::new);

    let begin = move |_| {
        open.set(true);
        if usage_count > 0 && detail().is_empty() {
            spawn(async move {
                if let Ok(list) = media_usage(id).await {
                    if !list.is_empty() {
                        let lines = list
                            .iter()
                            .map(|p| format!("• {} ({})", p.title, p.kind))
                            .collect::<Vec<_>>()
                            .join("\n");
                        detail.set(lines);
                    }
                }
            });
        }
    };

    let description = if usage_count > 0 {
        format!("This image is used in {usage_count} post(s) — they will show a broken image. This can't be undone.")
    } else {
        "This permanently deletes the image and can't be undone.".to_string()
    };

    rsx! {
        Button {
            variant: ButtonVariant::Destructive,
            size: ButtonSize::Xs,
            class: "shrink-0",
            title: "Delete",
            onclick: begin,
            "✕"
        }
        AlertDialog {
            open: open(),
            on_open_change: move |v| open.set(v),
            AlertDialogTitle { "Delete image?" }
            AlertDialogDescription {
                "{description}"
                if !detail().is_empty() {
                    pre { class: "mt-2 whitespace-pre-wrap text-xs text-white/60", "{detail}" }
                }
            }
            AlertDialogActions {
                AlertDialogCancel { "Cancel" }
                AlertDialogAction {
                    on_click: move |_| {
                        spawn(async move {
                            if delete_media(id).await.is_ok() {
                                on_deleted.call(());
                            }
                        });
                    },
                    "Delete"
                }
            }
        }
    }
}
