//! Post list (filter/sort table) and the Markdown editor with live preview.

use dioxus::prelude::*;
use dioxus_sdk_time::use_debounce;
use std::time::Duration;

use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::input::Input;
use crate::components::select::{Select, SelectOption};
use crate::components::surface::{Badge, BadgeTone, BadgeVariant};
use crate::components::text::{ErrorText, Eyebrow, EyebrowAs, EyebrowSize, Mb, PageTitle};
use crate::components::textarea::Textarea;
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
                PageTitle { mb: Mb::None, "Posts" }
                Button {
                    variant: ButtonVariant::Primary,
                    size: ButtonSize::Sm,
                    onclick: move |_| { navigator().push(Route::AdminPostNew); },
                    "New post"
                }
            }
            div { class: "mb-4 flex items-center gap-3 text-sm",
                label { class: "text-white/50", "Status" }
                Select::<String> {
                    default_value: Some(status_filter()),
                    on_value_change: move |v: Option<String>| status_filter.set(v.unwrap_or_default()),
                    SelectOption::<String> { index: 0usize, value: String::new(), "All" }
                    {POST_STATUSES.iter().enumerate().map(|(i, s)| rsx! {
                        SelectOption::<String> { key: "{s}", index: i + 1, value: s.to_string(), "{s}" }
                    })}
                }
                label { class: "ml-3 text-white/50", "Sort" }
                Select::<String> {
                    default_value: Some(sort()),
                    on_value_change: move |v: Option<String>| { if let Some(v) = v { sort.set(v); } },
                    SelectOption::<String> { index: 0usize, value: "recent".to_string(), "Recently updated" }
                    SelectOption::<String> { index: 1usize, value: "oldest".to_string(), "Oldest updated" }
                    SelectOption::<String> { index: 2usize, value: "title".to_string(), "Title A–Z" }
                    SelectOption::<String> { index: 3usize, value: "title_desc".to_string(), "Title Z–A" }
                    SelectOption::<String> { index: 4usize, value: "status".to_string(), "Status" }
                    SelectOption::<String> { index: 5usize, value: "published".to_string(), "Published date" }
                }
            }
            {list_states!(posts, empty: "No posts yet.", list => rsx! {
                        table { class: "w-full text-left text-sm",
                            thead { class: "border-b border-white/10 text-white/50",
                                tr {
                                    th { class: "py-2",
                                        Button { variant: ButtonVariant::Ghost, size: ButtonSize::Xs,
                                            onclick: move |_| toggle_sort("title", "title_desc"), "Title" }
                                    }
                                    th {
                                        Button { variant: ButtonVariant::Ghost, size: ButtonSize::Xs,
                                            onclick: move |_| toggle_sort("status", "status_desc"), "Status" }
                                    }
                                    th {
                                        Button { variant: ButtonVariant::Ghost, size: ButtonSize::Xs,
                                            onclick: move |_| toggle_sort("published", "published_desc"), "Published" }
                                    }
                                    th { "" }
                                }
                            }
                            tbody {
                                for p in list {
                                    tr { key: "{p.id}", class: "border-b border-white/5",
                                        td { class: "py-2", "{p.title}" }
                                        td { Badge { tone: BadgeTone::Neutral, variant: BadgeVariant::Outlined, "{p.status}" } }
                                        td { class: "text-white/50", {p.published_at.as_ref().map(crate::model::fmt_date).unwrap_or_else(|| "—".into())} }
                                        td { class: "flex gap-3 py-2",
                                            {
                                                let pid = p.id;
                                                rsx! {
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Xs,
                                                        onclick: move |_| { navigator().push(Route::AdminPostEdit { id: pid }); },
                                                        "Edit"
                                                    }
                                                    ActionButton {
                                                        label: "Delete".to_string(),
                                                        variant: ButtonVariant::Destructive,
                                                        confirm: Some("This permanently deletes the post and can't be undone.".to_string()),
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
                Some(Err(e)) => rsx! { ErrorText { "{e}" } },
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

    // Full-screen mode swaps the two-column form for a distraction-free overlay
    // holding just the editor + preview. Both layouts share the same `body`
    // signal, so the textarea and preview render through these closures rather
    // than being duplicated — toggling never loses in-progress text.
    let mut fullscreen = use_signal(|| false);
    let render_textarea = move |class: &'static str| {
        rsx! {
            Textarea {
                class,
                placeholder: "Write in Markdown…",
                value: "{body}",
                oninput: move |e: FormEvent| body.set(e.value()),
                onkeydown: move |e: KeyboardEvent| {
                    if e.key() == Key::Escape && fullscreen() {
                        fullscreen.set(false);
                    }
                },
            }
        }
    };
    let render_preview = move || {
        rsx! {
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
    };

    let post_id = initial.id;
    let submit = move |_| {
        let (t, ex, b, f) = (title(), excerpt(), body(), featured());
        let cat = category_id();
        let st = status();
        let tag_ids = selected_tags();
        let feat = if f.trim().is_empty() { None } else { Some(f) };
        let input = crate::model::PostInput {
            title: t,
            body_md: b,
            excerpt: ex,
            category_id: cat,
            tag_ids,
            featured_image_url: feat,
            status: st,
        };
        spawn(async move {
            let result = if editing {
                update_post(post_id, input).await.map(|_| post_id)
            } else {
                create_post(input).await
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
        PageTitle { if editing { "Edit post" } else { "New post" } }
        div { class: "grid gap-6 lg:grid-cols-2",
            // Left: form
            div { class: "space-y-3",
                Input {
                    class: "w-full text-lg font-semibold",
                    placeholder: "Title",
                    value: "{title}",
                    oninput: move |e: FormEvent| title.set(e.value()),
                }
                Input {
                    class: "w-full text-sm",
                    placeholder: "Excerpt",
                    value: "{excerpt}",
                    oninput: move |e: FormEvent| excerpt.set(e.value()),
                }
                // Featured image: URL field plus a media-library picker.
                div { class: "space-y-2",
                    div { class: "flex gap-2",
                        Input {
                            class: "flex-1 text-sm",
                            placeholder: "Featured image URL",
                            value: "{featured}",
                            oninput: move |e: FormEvent| {
                                let v = e.value();
                                featured.set(v.clone());
                                debounce_featured.action(v);
                            },
                        }
                        Button {
                            r#type: "button",
                            variant: ButtonVariant::Outline,
                            size: ButtonSize::Sm,
                            class: "shrink-0",
                            onclick: move |_| show_media_picker.set(!show_media_picker()),
                            if show_media_picker() { "Close" } else { "Library" }
                        }
                    }
                    if !featured_preview().trim().is_empty() {
                        img { class: "h-24 rounded-lg border border-white/10 object-cover", src: "{featured_preview}", alt: "Featured preview" }
                    }
                    if show_media_picker() {
                        div { class: "rounded-lg border border-white/10 bg-white/[0.03] p-2",
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
                                                        class: "overflow-hidden rounded-lg border border-white/10 hover:border-brand-400",
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
                                Some(Err(e)) => rsx! { ErrorText { small: true, class: "p-2".to_string(), "Error: {e}" } },
                                None => rsx! { p { class: "p-2 text-sm text-white/50", "Loading…" } },
                            }
                        }
                    }
                }
                div { class: "flex gap-3",
                    label { class: "flex-1 space-y-1",
                        span { class: "block text-sm font-medium text-white/70", "Category" }
                        Select::<String> {
                            default_value: Some(category_id().map(|id| id.to_string()).unwrap_or_default()),
                            on_value_change: move |v: Option<String>| {
                                category_id.set(v.and_then(|s| s.parse::<i64>().ok()))
                            },
                            SelectOption::<String> { index: 0usize, value: String::new(), "Uncategorized" }
                            if let Some(Ok(list)) = &*cats.read() {
                                {list.clone().into_iter().enumerate().map(|(i, c)| rsx! {
                                    SelectOption::<String> { key: "{c.id}", index: i + 1, value: "{c.id}", "{c.name}" }
                                })}
                            }
                        }
                    }
                    label { class: "flex-1 space-y-1",
                        span { class: "block text-sm font-medium text-white/70", "Status" }
                        Select::<String> {
                            default_value: Some(status()),
                            on_value_change: move |v: Option<String>| { if let Some(v) = v { status.set(v); } },
                            {POST_STATUSES.iter().enumerate().map(|(i, s)| rsx! {
                                SelectOption::<String> { key: "{s}", index: i, value: s.to_string(), "{s}" }
                            })}
                        }
                    }
                }
                if let Some(Ok(list)) = &*tags.read() {
                    div { class: "flex flex-wrap gap-2",
                        for t in list.clone() {
                            {
                                let tid = t.id;
                                let active = selected_tags().contains(&tid);
                                rsx! {
                                    // Toggle pill: brand-filled when selected, outline when not.
                                    Button {
                                        key: "{t.id}",
                                        r#type: "button",
                                        variant: if active { ButtonVariant::Primary } else { ButtonVariant::Outline },
                                        size: ButtonSize::Xs,
                                        onclick: move |_| {
                                            let mut cur = selected_tags();
                                            if cur.contains(&tid) {
                                                cur.retain(|x| *x != tid);
                                            } else {
                                                cur.push(tid);
                                            }
                                            selected_tags.set(cur);
                                        },
                                        "#{t.name}"
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "flex items-center justify-between",
                    Eyebrow { r#as: EyebrowAs::Label, size: EyebrowSize::Sm, "Markdown" }
                    Button {
                        r#type: "button",
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Xs,
                        onclick: move |_| fullscreen.set(true),
                        "⤢ Full screen"
                    }
                }
                {render_textarea("h-80 w-full font-mono text-sm")}
                div { class: "flex items-center gap-3",
                    Button {
                        variant: ButtonVariant::Primary,
                        size: ButtonSize::Sm,
                        onclick: submit,
                        if editing { "Save changes" } else { "Create post" }
                    }
                    if !msg().is_empty() {
                        ErrorText { inline: true, small: true, "{msg}" }
                    }
                }
            }
            // Right: live preview
            div {
                Eyebrow { r#as: EyebrowAs::Div, size: EyebrowSize::Sm, mb: Mb::Mb2, "Preview" }
                div { class: "prose max-w-none rounded-lg border border-white/10 p-4",
                    {render_preview()}
                }
            }
        }
        if fullscreen() {
            // Distraction-free overlay: editor + preview fill the viewport.
            div { class: "fixed inset-0 z-50 flex flex-col bg-[#0f1116]",
                div { class: "flex shrink-0 items-center justify-between border-b border-white/10 px-4 py-2",
                    span { class: "text-sm text-white/60", if editing { "Edit post" } else { "New post" } }
                    Button {
                        r#type: "button",
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Sm,
                        onclick: move |_| fullscreen.set(false),
                        "Exit full screen (Esc)"
                    }
                }
                div { class: "grid min-h-0 flex-1 grid-cols-1 lg:grid-cols-2",
                    {render_textarea("h-full w-full resize-none border-r border-white/10 font-mono text-sm")}
                    div { class: "prose max-w-none overflow-y-auto p-4",
                        {render_preview()}
                    }
                }
            }
        }
    }
}
