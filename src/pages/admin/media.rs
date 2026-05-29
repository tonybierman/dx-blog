//! Media library: upload an image (read as base64 in the browser, posted to the
//! server) and browse / copy-url / delete existing uploads.

use dioxus::prelude::*;

use crate::pages::widgets::list_states;
use crate::server::admin::{delete_media, list_media, upload_media};

use super::{ActionButton, ActionFuture, AdminShell};

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
            h1 { class: "mb-6 text-2xl font-bold", "Media library" }
            div { class: "mb-6 flex items-center gap-3",
                input { id: "mediafile", r#type: "file", accept: "image/*", class: "text-sm" }
                button { class: "rounded bg-brand-600 px-3 py-1.5 text-sm font-medium hover:bg-brand-500", onclick: upload, "Upload" }
                if !msg().is_empty() { span { class: "text-sm text-white/60", "{msg}" } }
            }
            {list_states!(media, empty: "No media yet.", list => rsx! {
                    div { class: "columns-2 gap-4 md:columns-3 lg:columns-4",
                        for m in list {
                            div { key: "{m.id}", class: "mb-4 inline-block w-full break-inside-avoid rounded-lg border border-white/10 p-2",
                                img { class: "w-full rounded", src: "{m.url}", alt: "{m.filename}" }
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
                                    {
                                        let mid = m.id;
                                        rsx! {
                                            ActionButton {
                                                label: "✕".to_string(),
                                                class: "text-xs text-red-400 hover:underline".to_string(),
                                                on_done: move |_| media.restart(),
                                                action: move |_| Box::pin(async move { delete_media(mid).await }) as ActionFuture,
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
            })}
        }
    }
}
