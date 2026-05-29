//! Post list (filter/sort table) and the Markdown editor with live preview.

use dioxus::prelude::*;
use dioxus_sdk_time::use_debounce;
use std::time::Duration;

use crate::model::{PostEditData, POST_STATUSES, STATUS_DRAFT};
use crate::pages::widgets::list_states;
use crate::server::admin::{
    admin_list_posts, create_post, delete_post, get_post_edit, list_media, update_post,
};
use crate::server::taxonomy::{list_categories, list_tags};
use crate::Route;

use super::{ActionButton, ActionFuture, AdminShell};

#[component]
pub fn AdminPostList() -> Element {
    // Filter (status) + sort state; the resource refetches whenever either changes.
    let mut status_filter = use_signal(String::new);
    let mut sort = use_signal(|| "recent".to_string());
    let mut posts = use_resource(move || {
        let st = status_filter();
        let so = sort();
        async move {
            let st = if st.is_empty() { None } else { Some(st) };
            admin_list_posts(st, Some(so)).await
        }
    });

    // Clicking a sortable header toggles between its asc/desc variants.
    let mut toggle_sort = move |asc: &str, desc: &str| {
        let cur = sort();
        sort.set(if cur == asc {
            desc.to_string()
        } else {
            asc.to_string()
        });
    };

    rsx! {
        AdminShell { active: "posts".to_string(),
            div { class: "mb-6 flex items-center justify-between",
                h1 { class: "text-2xl font-bold", "Posts" }
                Link { to: Route::AdminPostNew, class: "rounded bg-brand-600 px-3 py-1.5 text-sm font-medium hover:bg-brand-500", "New post" }
            }
            div { class: "mb-4 flex items-center gap-3 text-sm",
                label { class: "text-white/50", "Status" }
                select {
                    class: "rounded border border-white/15 bg-transparent px-2 py-1.5",
                    onchange: move |e| status_filter.set(e.value()),
                    option { value: "", selected: status_filter().is_empty(), "All" }
                    for s in POST_STATUSES {
                        option { value: "{s}", selected: status_filter() == s, "{s}" }
                    }
                }
                label { class: "ml-3 text-white/50", "Sort" }
                select {
                    class: "rounded border border-white/15 bg-transparent px-2 py-1.5",
                    onchange: move |e| sort.set(e.value()),
                    option { value: "recent", selected: sort() == "recent", "Recently updated" }
                    option { value: "oldest", selected: sort() == "oldest", "Oldest updated" }
                    option { value: "title", selected: sort() == "title", "Title A–Z" }
                    option { value: "title_desc", selected: sort() == "title_desc", "Title Z–A" }
                    option { value: "status", selected: sort() == "status", "Status" }
                    option { value: "published", selected: sort() == "published", "Published date" }
                }
            }
            {list_states!(posts, empty: "No posts yet.", list => rsx! {
                        table { class: "w-full text-left text-sm",
                            thead { class: "border-b border-white/10 text-white/50",
                                tr {
                                    th { class: "py-2",
                                        button { class: "font-medium hover:text-white",
                                            onclick: move |_| toggle_sort("title", "title_desc"), "Title" }
                                    }
                                    th {
                                        button { class: "font-medium hover:text-white",
                                            onclick: move |_| toggle_sort("status", "status_desc"), "Status" }
                                    }
                                    th {
                                        button { class: "font-medium hover:text-white",
                                            onclick: move |_| toggle_sort("published", "published_desc"), "Published" }
                                    }
                                    th { "" }
                                }
                            }
                            tbody {
                                for p in list {
                                    tr { key: "{p.id}", class: "border-b border-white/5",
                                        td { class: "py-2", "{p.title}" }
                                        td { span { class: "rounded-full border border-white/15 px-2 py-0.5 text-xs", "{p.status}" } }
                                        td { class: "text-white/50", {p.published_at.clone().unwrap_or_else(|| "—".into())} }
                                        td { class: "flex gap-3 py-2",
                                            Link { to: Route::AdminPostEdit { id: p.id }, class: "text-brand-400 hover:underline", "Edit" }
                                            {
                                                let pid = p.id;
                                                rsx! {
                                                    ActionButton {
                                                        label: "Delete".to_string(),
                                                        class: "text-red-400 hover:underline".to_string(),
                                                        on_done: move |_| posts.restart(),
                                                        action: move |_| Box::pin(async move { delete_post(pid).await }) as ActionFuture,
                                                    }
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

#[component]
pub fn AdminPostNew() -> Element {
    rsx! {
        AdminShell { active: "new".to_string(),
            EditorForm { initial: PostEditData { status: STATUS_DRAFT.to_string(), ..Default::default() } }
        }
    }
}

#[component]
pub fn AdminPostEdit(id: i64) -> Element {
    let data = use_resource(use_reactive!(
        |(id,)| async move { get_post_edit(id).await }
    ));
    rsx! {
        AdminShell { active: "posts".to_string(),
            match &*data.read() {
                Some(Ok(d)) => rsx! { EditorForm { key: "{d.id}", initial: d.clone() } },
                Some(Err(e)) => rsx! { p { class: "text-red-400", "{e}" } },
                None => rsx! { p { class: "text-white/50", "Loading…" } },
            }
        }
    }
}

#[component]
fn EditorForm(initial: PostEditData) -> Element {
    let editing = initial.id != 0;
    let mut title = use_signal(|| initial.title.clone());
    let mut excerpt = use_signal(|| initial.excerpt.clone());
    let mut body = use_signal(|| initial.body_md.clone());
    let mut featured = use_signal(|| initial.featured_image_url.clone().unwrap_or_default());
    // The featured-image thumbnail keys off a debounced copy of `featured` so it
    // doesn't fire an image request (and flash a broken icon) on every keystroke
    // while a URL is being typed — same pattern as the markdown preview below.
    let mut featured_preview =
        use_signal(|| initial.featured_image_url.clone().unwrap_or_default());
    let mut debounce_featured = use_debounce(Duration::from_millis(400), move |v: String| {
        featured_preview.set(v)
    });
    let mut category_id = use_signal(|| initial.category_id);
    let mut status = use_signal(|| initial.status.clone());
    let mut selected_tags = use_signal(|| initial.tag_ids.clone());
    let mut msg = use_signal(String::new);

    let cats = use_resource(list_categories);
    let tags = use_resource(list_tags);
    // Media library for the featured-image picker.
    let media = use_resource(list_media);
    let mut show_media_picker = use_signal(|| false);

    // Live preview renders in the browser (WASM): `parse_body` runs the same
    // pulldown-cmark + ammonia pipeline the server uses to produce stored
    // `body_html`, so prose is byte-for-byte what gets saved, and it mounts the
    // same live "Rust MDX" embed components the reader does — authors see
    // interactive blocks as they type. Running it locally means zero-latency,
    // offline-capable preview with no server load, so no debounce is needed;
    // `use_memo` recomputes synchronously per keystroke.
    let preview = use_memo(move || crate::mdx::parse_body(&body()));

    let post_id = initial.id;
    let submit = move |_| {
        let (t, ex, b, f) = (title(), excerpt(), body(), featured());
        let cat = category_id();
        let st = status();
        let tag_ids = selected_tags();
        let feat = if f.trim().is_empty() { None } else { Some(f) };
        spawn(async move {
            let result = if editing {
                update_post(post_id, t, b, ex, cat, tag_ids, feat, st)
                    .await
                    .map(|_| post_id)
            } else {
                create_post(t, b, ex, cat, tag_ids, feat, st).await
            };
            match result {
                Ok(_) => {
                    navigator().push(Route::AdminPostList);
                }
                Err(e) => msg.set(arium_dioxus::friendly_server_error(e)),
            }
        });
    };

    rsx! {
        h1 { class: "mb-6 text-2xl font-bold", if editing { "Edit post" } else { "New post" } }
        div { class: "grid gap-6 lg:grid-cols-2",
            // Left: form
            div { class: "space-y-3",
                input {
                    class: "w-full rounded border border-white/15 bg-transparent px-3 py-2 text-lg font-semibold",
                    placeholder: "Title",
                    value: "{title}",
                    oninput: move |e| title.set(e.value()),
                }
                input {
                    class: "w-full rounded border border-white/15 bg-transparent px-3 py-2 text-sm",
                    placeholder: "Excerpt",
                    value: "{excerpt}",
                    oninput: move |e| excerpt.set(e.value()),
                }
                // Featured image: URL field plus a media-library picker.
                div { class: "space-y-2",
                    div { class: "flex gap-2",
                        input {
                            class: "flex-1 rounded border border-white/15 bg-transparent px-3 py-2 text-sm",
                            placeholder: "Featured image URL",
                            value: "{featured}",
                            oninput: move |e| {
                                let v = e.value();
                                featured.set(v.clone());
                                debounce_featured.action(v);
                            },
                        }
                        button {
                            r#type: "button",
                            class: "shrink-0 rounded border border-white/15 px-3 text-sm hover:bg-white/5",
                            onclick: move |_| show_media_picker.set(!show_media_picker()),
                            if show_media_picker() { "Close" } else { "Library" }
                        }
                    }
                    if !featured_preview().trim().is_empty() {
                        img { class: "h-24 rounded border border-white/10 object-cover", src: "{featured_preview}", alt: "Featured preview" }
                    }
                    if show_media_picker() {
                        div { class: "rounded border border-white/10 bg-white/[0.03] p-2",
                            match &*media.read() {
                                Some(Ok(list)) if !list.is_empty() => rsx! {
                                    div { class: "grid max-h-48 grid-cols-4 gap-2 overflow-y-auto sm:grid-cols-6",
                                        for m in list.clone() {
                                            {
                                                let url = m.url.clone();
                                                rsx! {
                                                    button {
                                                        key: "{m.id}",
                                                        r#type: "button",
                                                        class: "overflow-hidden rounded border border-white/10 hover:border-brand-400",
                                                        title: "{m.filename}",
                                                        onclick: move |_| { featured.set(url.clone()); featured_preview.set(url.clone()); show_media_picker.set(false); },
                                                        img { class: "h-16 w-full object-cover", src: "{m.url}", alt: "{m.filename}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                },
                                Some(Ok(_)) => rsx! { p { class: "p-2 text-sm text-white/50", "No media uploaded yet — add some on the Media page." } },
                                Some(Err(e)) => rsx! { p { class: "p-2 text-sm text-red-400", "Error: {e}" } },
                                None => rsx! { p { class: "p-2 text-sm text-white/50", "Loading…" } },
                            }
                        }
                    }
                }
                div { class: "flex gap-3",
                    select {
                        class: "rounded border border-white/15 bg-transparent px-2 py-1.5 text-sm",
                        onchange: move |e| category_id.set(e.value().parse::<i64>().ok()),
                        option { value: "", "— Category —" }
                        if let Some(Ok(list)) = &*cats.read() {
                            for c in list.clone() {
                                option { value: "{c.id}", selected: category_id() == Some(c.id), "{c.name}" }
                            }
                        }
                    }
                    select {
                        class: "rounded border border-white/15 bg-transparent px-2 py-1.5 text-sm",
                        onchange: move |e| status.set(e.value()),
                        for s in POST_STATUSES {
                            option { value: "{s}", selected: status() == s, "{s}" }
                        }
                    }
                }
                if let Some(Ok(list)) = &*tags.read() {
                    div { class: "flex flex-wrap gap-2",
                        for t in list.clone() {
                            label { key: "{t.id}", class: "flex items-center gap-1 text-xs text-white/70",
                                input {
                                    r#type: "checkbox",
                                    checked: selected_tags().contains(&t.id),
                                    onchange: move |e| {
                                        let mut cur = selected_tags();
                                        if e.checked() { if !cur.contains(&t.id) { cur.push(t.id); } }
                                        else { cur.retain(|x| *x != t.id); }
                                        selected_tags.set(cur);
                                    },
                                }
                                "#{t.name}"
                            }
                        }
                    }
                }
                textarea {
                    class: "h-80 w-full rounded border border-white/15 bg-transparent px-3 py-2 font-mono text-sm",
                    placeholder: "Write in Markdown…",
                    value: "{body}",
                    oninput: move |e| body.set(e.value()),
                }
                div { class: "flex items-center gap-3",
                    button {
                        class: "rounded bg-brand-600 px-4 py-2 text-sm font-medium hover:bg-brand-500",
                        onclick: submit,
                        if editing { "Save changes" } else { "Create post" }
                    }
                    if !msg().is_empty() {
                        span { class: "text-sm text-red-400", "{msg}" }
                    }
                }
            }
            // Right: live preview
            div {
                h3 { class: "mb-2 text-sm uppercase tracking-wide text-white/40", "Preview" }
                div { class: "prose max-w-none rounded border border-white/10 p-4",
                    for (i, seg) in preview().into_iter().enumerate() {
                        match seg {
                            crate::mdx::Segment::Html(html) => rsx! {
                                div { key: "{i}", dangerous_inner_html: "{html}" }
                            },
                            crate::mdx::Segment::Embed { name, props } => rsx! {
                                crate::embeds::EmbedBlock { key: "{i}", name, props }
                            },
                        }
                    }
                }
            }
        }
    }
}
