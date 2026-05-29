//! Admin & authoring surface. The server fns are the real authorization
//! boundary; `RequirePermission` here just keeps unauthorized users out of the
//! UI and redirects them to sign in.

use dioxus::prelude::*;
use dioxus_sdk_time::use_debounce;
use std::time::Duration;

use arium_dioxus::ui::{Policy, RequirePermission};

use crate::auth_tokens::{
    ANALYTICS_READ, COMMENTS_MODERATE, POSTS_WRITE, POSTS_WRITE_ANY, SETTINGS_WRITE, USERS_MANAGE,
};
use crate::model::{AnalyticsSummary, PostEditData};
use crate::server::admin::*;
use crate::server::analytics::{analytics_summary, top_posts, top_referrers, views_over_time};
use crate::server::taxonomy::{list_categories, list_tags};
use crate::Route;

fn admin_any_policy() -> Policy {
    Policy::any_of([
        POSTS_WRITE,
        POSTS_WRITE_ANY,
        COMMENTS_MODERATE,
        USERS_MANAGE,
        SETTINGS_WRITE,
        ANALYTICS_READ,
    ])
}

fn nav_class(active: &str, name: &str) -> &'static str {
    if active == name {
        "rounded bg-white/10 px-3 py-1.5 font-medium"
    } else {
        "rounded px-3 py-1.5 text-white/60 hover:bg-white/5 hover:text-white"
    }
}

#[component]
fn AdminShell(active: String, children: Element) -> Element {
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
                        Link { to: Route::AdminAnalytics, class: nav_class(&active, "analytics"), "Analytics" }
                    }
                    Link { to: Route::HomePage, class: "mt-6 block text-xs text-white/40 hover:underline", "← Back to site" }
                }
                main { class: "flex-1 p-6", {children} }
            }
        }
    }
}

// ---------------------------------------------------------------- dashboard / analytics

#[component]
fn MetricTiles(summary: AnalyticsSummary) -> Element {
    let tiles = [
        ("Posts", summary.post_count),
        ("Published", summary.published_count),
        ("Drafts", summary.draft_count),
        ("Comments", summary.comment_count),
        ("Pending", summary.pending_comment_count),
        ("Subscribers", summary.subscriber_count),
        ("Views", summary.view_count),
    ];
    rsx! {
        div { class: "grid auto-rows-[120px] grid-cols-2 gap-4 md:grid-cols-4",
            for (i, (label, value)) in tiles.into_iter().enumerate() {
                div {
                    key: "{label}",
                    class: if i == 0 { "col-span-2 row-span-2 rounded-xl border border-white/10 bg-white/[0.03] p-4 flex flex-col justify-center" } else { "rounded-xl border border-white/10 bg-white/[0.03] p-4 flex flex-col justify-center" },
                    div { class: "text-sm text-white/50", "{label}" }
                    div { class: if i == 0 { "text-5xl font-bold" } else { "text-3xl font-bold" }, "{value}" }
                }
            }
        }
    }
}

#[component]
pub fn AdminDashboard() -> Element {
    let summary = use_resource(analytics_summary);
    rsx! {
        AdminShell { active: "dashboard".to_string(),
            h1 { class: "mb-6 text-2xl font-bold", "Dashboard" }
            match &*summary.read() {
                Some(Ok(s)) => rsx! { MetricTiles { summary: s.clone() } },
                Some(Err(e)) => rsx! { p { class: "text-red-400", "{e}" } },
                None => rsx! { p { class: "text-white/50", "Loading…" } },
            }
        }
    }
}

#[component]
pub fn AdminAnalytics() -> Element {
    let summary = use_resource(analytics_summary);
    let top = use_resource(top_posts);
    let referrers = use_resource(top_referrers);
    let series = use_resource(views_over_time);
    rsx! {
        AdminShell { active: "analytics".to_string(),
            h1 { class: "mb-6 text-2xl font-bold", "Analytics" }
            match &*summary.read() {
                Some(Ok(s)) => rsx! { MetricTiles { summary: s.clone() } },
                Some(Err(e)) => rsx! { p { class: "text-red-400", "{e}" } },
                None => rsx! { p { class: "text-white/50", "Loading…" } },
            }

            h2 { class: "mb-3 mt-8 text-lg font-semibold", "Views over time" }
            div { class: "rounded-xl border border-white/10 bg-white/[0.03] p-4",
                match &*series.read() {
                    Some(Ok(days)) if !days.is_empty() => {
                        let days = days.clone();
                        let peak = days.iter().map(|d| d.views).max().unwrap_or(1).max(1);
                        rsx! {
                            div { class: "flex h-40 items-end gap-1",
                                for d in days {
                                    div {
                                        key: "{d.day}",
                                        class: "group relative flex-1 rounded-t bg-brand-500/70 hover:bg-brand-400",
                                        style: "height: {(d.views * 100) / peak}%",
                                        title: "{d.day}: {d.views} views",
                                    }
                                }
                            }
                        }
                    }
                    Some(Ok(_)) => rsx! { p { class: "text-white/40", "No views in the last 30 days." } },
                    Some(Err(e)) => rsx! { p { class: "text-red-400", "{e}" } },
                    None => rsx! { p { class: "text-white/50", "…" } },
                }
            }

            div { class: "mt-8 grid gap-8 md:grid-cols-2",
                section {
                    h2 { class: "mb-3 text-lg font-semibold", "Top posts" }
                    match &*top.read() {
                        Some(Ok(list)) if !list.is_empty() => rsx! {
                            ul { class: "space-y-1 text-sm",
                                for p in list.clone() {
                                    li { key: "{p.id}", class: "text-white/70", "{p.title}" }
                                }
                            }
                        },
                        Some(Ok(_)) => rsx! { p { class: "text-white/40", "No views yet." } },
                        Some(Err(e)) => rsx! { p { class: "text-red-400", "{e}" } },
                        None => rsx! { p { class: "text-white/50", "…" } },
                    }
                }
                section {
                    h2 { class: "mb-3 text-lg font-semibold", "Top referrers" }
                    match &*referrers.read() {
                        Some(Ok(list)) if !list.is_empty() => rsx! {
                            ul { class: "space-y-1 text-sm",
                                for (i, r) in list.clone().into_iter().enumerate() {
                                    li { key: "{i}", class: "flex items-center justify-between gap-3",
                                        span { class: "truncate text-white/70", title: "{r.referrer}", "{r.referrer}" }
                                        span { class: "shrink-0 tabular-nums text-white/50", "{r.views}" }
                                    }
                                }
                            }
                        },
                        Some(Ok(_)) => rsx! { p { class: "text-white/40", "No referrers yet." } },
                        Some(Err(e)) => rsx! { p { class: "text-red-400", "{e}" } },
                        None => rsx! { p { class: "text-white/50", "…" } },
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------- post list

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
        sort.set(if cur == asc { desc.to_string() } else { asc.to_string() });
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
                    for s in ["draft", "published", "archived"] {
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
            match &*posts.read() {
                Some(Ok(list)) if !list.is_empty() => {
                    let list = list.clone();
                    rsx! {
                        table { class: "w-full text-left text-sm",
                            thead { class: "border-b border-white/10 text-white/50",
                                tr {
                                    th { class: "py-2",
                                        button { class: "font-medium hover:text-white",
                                            onclick: move |_| toggle_sort("title", "title_desc"), "Title" }
                                    }
                                    th {
                                        button { class: "font-medium hover:text-white",
                                            onclick: move |_| toggle_sort("status", "status"), "Status" }
                                    }
                                    th {
                                        button { class: "font-medium hover:text-white",
                                            onclick: move |_| toggle_sort("published", "published"), "Published" }
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
                                            DeletePostButton { id: p.id, on_deleted: move |_| posts.restart() }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Some(Ok(_)) => rsx! { p { class: "text-white/50", "No posts yet." } },
                Some(Err(e)) => rsx! { p { class: "text-red-400", "{e}" } },
                None => rsx! { p { class: "text-white/50", "Loading…" } },
            }
        }
    }
}

#[component]
fn DeletePostButton(id: i64, on_deleted: EventHandler<()>) -> Element {
    rsx! {
        button {
            class: "text-red-400 hover:underline",
            onclick: move |_| {
                spawn(async move {
                    if delete_post(id).await.is_ok() {
                        on_deleted.call(());
                    }
                });
            },
            "Delete"
        }
    }
}

// ---------------------------------------------------------------- editor

#[component]
pub fn AdminPostNew() -> Element {
    rsx! {
        AdminShell { active: "new".to_string(),
            EditorForm { initial: PostEditData { status: "draft".to_string(), ..Default::default() } }
        }
    }
}

#[component]
pub fn AdminPostEdit(id: i64) -> Element {
    let data = use_resource(use_reactive!(|(id,)| async move { get_post_edit(id).await }));
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
    let mut category_id = use_signal(|| initial.category_id);
    let mut status = use_signal(|| initial.status.clone());
    let mut selected_tags = use_signal(|| initial.tag_ids.clone());
    let mut msg = use_signal(String::new);

    let cats = use_resource(list_categories);
    let tags = use_resource(list_tags);
    // Media library for the featured-image picker.
    let media = use_resource(list_media);
    let mut show_media_picker = use_signal(|| false);

    // Live preview is debounced: the textarea updates `body` on every keystroke
    // (instant local echo), but the server round-trip keys off `preview_md`,
    // which only catches up 400ms after the user stops typing.
    let mut preview_md = use_signal(|| initial.body_md.clone());
    let mut debounce_preview = use_debounce(Duration::from_millis(400), move |md: String| {
        preview_md.set(md);
    });
    let preview = use_resource(move || {
        let md = preview_md();
        async move { preview_markdown(md).await }
    });

    let post_id = initial.id;
    let submit = move |_| {
        let (t, ex, b, f) = (title(), excerpt(), body(), featured());
        let cat = category_id();
        let st = status();
        let tag_ids = selected_tags();
        let feat = if f.trim().is_empty() { None } else { Some(f) };
        spawn(async move {
            let result = if editing {
                update_post(post_id, t, b, ex, cat, tag_ids, feat, st).await.map(|_| post_id)
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
                            oninput: move |e| featured.set(e.value()),
                        }
                        button {
                            r#type: "button",
                            class: "shrink-0 rounded border border-white/15 px-3 text-sm hover:bg-white/5",
                            onclick: move |_| show_media_picker.set(!show_media_picker()),
                            if show_media_picker() { "Close" } else { "Library" }
                        }
                    }
                    if !featured().trim().is_empty() {
                        img { class: "h-24 rounded border border-white/10 object-cover", src: "{featured}", alt: "Featured preview" }
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
                                                        onclick: move |_| { featured.set(url.clone()); show_media_picker.set(false); },
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
                        for s in ["draft", "published", "archived"] {
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
                    oninput: move |e| {
                        let v = e.value();
                        body.set(v.clone());
                        debounce_preview.action(v);
                    },
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
                div { class: "prose prose-invert max-w-none rounded border border-white/10 p-4",
                    match &*preview.read() {
                        Some(Ok(html)) => rsx! { div { dangerous_inner_html: "{html}" } },
                        _ => rsx! { p { class: "text-white/40", "…" } },
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------- comments

#[component]
pub fn AdminComments() -> Element {
    let mut comments = use_resource(move || async move { admin_list_comments(None).await });
    rsx! {
        AdminShell { active: "comments".to_string(),
            h1 { class: "mb-6 text-2xl font-bold", "Comment moderation" }
            match &*comments.read() {
                Some(Ok(list)) if !list.is_empty() => {
                    let list = list.clone();
                    rsx! {
                        div { class: "space-y-3",
                            for c in list {
                                div { key: "{c.id}", class: "rounded-lg border border-white/10 p-3",
                                    div { class: "flex items-center justify-between",
                                        div { class: "text-sm font-medium", "{c.display_name}" }
                                        span { class: "rounded-full border border-white/15 px-2 py-0.5 text-xs", "{c.status}" }
                                    }
                                    p { class: "mt-1 text-sm text-white/80", "{c.body}" }
                                    div { class: "mt-2 flex gap-3 text-xs",
                                        ModButton { id: c.id, action: "approved", label: "Approve", on_done: move |_| comments.restart() }
                                        ModButton { id: c.id, action: "rejected", label: "Reject", on_done: move |_| comments.restart() }
                                        DeleteCommentButton { id: c.id, on_done: move |_| comments.restart() }
                                    }
                                }
                            }
                        }
                    }
                }
                Some(Ok(_)) => rsx! { p { class: "text-white/50", "No comments." } },
                Some(Err(e)) => rsx! { p { class: "text-red-400", "{e}" } },
                None => rsx! { p { class: "text-white/50", "Loading…" } },
            }
        }
    }
}

#[component]
fn ModButton(id: i64, action: String, label: String, on_done: EventHandler<()>) -> Element {
    rsx! {
        button {
            class: "text-brand-400 hover:underline",
            onclick: move |_| {
                let action = action.clone();
                spawn(async move {
                    if moderate_comment(id, action).await.is_ok() { on_done.call(()); }
                });
            },
            "{label}"
        }
    }
}

#[component]
fn DeleteCommentButton(id: i64, on_done: EventHandler<()>) -> Element {
    rsx! {
        button {
            class: "text-red-400 hover:underline",
            onclick: move |_| {
                spawn(async move {
                    if delete_comment(id).await.is_ok() { on_done.call(()); }
                });
            },
            "Delete"
        }
    }
}

// ---------------------------------------------------------------- media

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
                    r.readAsDataURL(f);
                }
                "#,
            );
            match eval.recv::<String>().await {
                Ok(s) if !s.is_empty() => {
                    if let Some((name, b64)) = s.split_once('|') {
                        match upload_media(name.to_string(), b64.to_string()).await {
                            Ok(_) => { msg.set("Uploaded.".into()); media.restart(); }
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
            match &*media.read() {
                Some(Ok(list)) if !list.is_empty() => rsx! {
                    div { class: "columns-2 gap-4 md:columns-3 lg:columns-4",
                        for m in list.clone() {
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
                                    DeleteMediaButton { id: m.id, on_done: move |_| media.restart() }
                                }
                            }
                        }
                    }
                },
                Some(Ok(_)) => rsx! { p { class: "text-white/50", "No media yet." } },
                Some(Err(e)) => rsx! { p { class: "text-red-400", "{e}" } },
                None => rsx! { p { class: "text-white/50", "Loading…" } },
            }
        }
    }
}

#[component]
fn DeleteMediaButton(id: i64, on_done: EventHandler<()>) -> Element {
    rsx! {
        button {
            class: "text-xs text-red-400 hover:underline",
            onclick: move |_| {
                spawn(async move { if delete_media(id).await.is_ok() { on_done.call(()); } });
            },
            "✕"
        }
    }
}

// ---------------------------------------------------------------- users (arium UI)

#[component]
pub fn AdminUsers() -> Element {
    let mut selected = use_signal::<Option<i64>>(|| None);
    rsx! {
        AdminShell { active: "users".to_string(),
            h1 { class: "mb-6 text-2xl font-bold", "User management" }
            if let Some(uid) = selected() {
                arium_dioxus::ui::AdminUserDetail { user_id: uid, on_back: move |_| selected.set(None) }
            } else {
                arium_dioxus::ui::AdminUserList { on_select: move |id: i64| selected.set(Some(id)) }
            }
        }
    }
}

// ---------------------------------------------------------------- settings (taxonomy)

#[component]
pub fn AdminSettings() -> Element {
    let mut cats = use_resource(list_categories);
    let mut tags = use_resource(list_tags);
    let mut new_cat = use_signal(String::new);
    let mut new_tag = use_signal(String::new);
    // Inline-rename state: which row (by id) is being edited, and its draft name.
    let mut edit_cat = use_signal::<Option<i64>>(|| None);
    let mut edit_cat_name = use_signal(String::new);
    let mut edit_tag = use_signal::<Option<i64>>(|| None);
    let mut edit_tag_name = use_signal(String::new);

    let add_cat = move |_| {
        let name = new_cat();
        if name.trim().is_empty() { return; }
        spawn(async move {
            if create_category(name, None).await.is_ok() { new_cat.set(String::new()); cats.restart(); }
        });
    };
    let add_tag = move |_| {
        let name = new_tag();
        if name.trim().is_empty() { return; }
        spawn(async move {
            if create_tag(name).await.is_ok() { new_tag.set(String::new()); tags.restart(); }
        });
    };

    rsx! {
        AdminShell { active: "settings".to_string(),
            h1 { class: "mb-6 text-2xl font-bold", "Site settings" }
            div { class: "grid gap-8 md:grid-cols-2",
                section {
                    h2 { class: "mb-3 text-lg font-semibold", "Categories" }
                    div { class: "mb-3 flex gap-2",
                        input { class: "flex-1 rounded border border-white/15 bg-transparent px-2 py-1 text-sm", placeholder: "New category", value: "{new_cat}", oninput: move |e| new_cat.set(e.value()) }
                        button { class: "rounded bg-brand-600 px-3 text-sm", onclick: add_cat, "Add" }
                    }
                    if let Some(Ok(list)) = &*cats.read() {
                        ul { class: "space-y-1 text-sm",
                            for c in list.clone() {
                                {
                                    let cid = c.id;
                                    let cname = c.name.clone();
                                    let save = move |_| {
                                        let name = edit_cat_name();
                                        spawn(async move {
                                            if rename_category(cid, name).await.is_ok() {
                                                edit_cat.set(None);
                                                cats.restart();
                                            }
                                        });
                                    };
                                    rsx! {
                                        li { key: "{cid}", class: "flex items-center justify-between gap-2",
                                            if edit_cat() == Some(cid) {
                                                input {
                                                    class: "flex-1 rounded border border-white/15 bg-transparent px-2 py-1 text-sm",
                                                    value: "{edit_cat_name}",
                                                    oninput: move |e| edit_cat_name.set(e.value()),
                                                    onkeydown: move |e| if e.key() == Key::Enter { save(()) },
                                                }
                                                button { class: "text-xs text-brand-400 hover:underline", onclick: move |_| save(()), "Save" }
                                                button { class: "text-xs text-white/50 hover:underline",
                                                    onclick: move |_| edit_cat.set(None), "Cancel"
                                                }
                                            } else {
                                                span { "{cname}" }
                                                div { class: "flex gap-2",
                                                    button { class: "text-xs text-white/60 hover:underline",
                                                        onclick: move |_| { edit_cat_name.set(cname.clone()); edit_cat.set(Some(cid)); },
                                                        "Edit"
                                                    }
                                                    button { class: "text-xs text-red-400 hover:underline",
                                                        onclick: move |_| { spawn(async move { if delete_category(cid).await.is_ok() { cats.restart(); } }); },
                                                        "Delete"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                section {
                    h2 { class: "mb-3 text-lg font-semibold", "Tags" }
                    div { class: "mb-3 flex gap-2",
                        input { class: "flex-1 rounded border border-white/15 bg-transparent px-2 py-1 text-sm", placeholder: "New tag", value: "{new_tag}", oninput: move |e| new_tag.set(e.value()) }
                        button { class: "rounded bg-brand-600 px-3 text-sm", onclick: add_tag, "Add" }
                    }
                    if let Some(Ok(list)) = &*tags.read() {
                        div { class: "flex flex-wrap gap-2",
                            for t in list.clone() {
                                {
                                    let tid = t.id;
                                    let tname = t.name.clone();
                                    let save = move |_| {
                                        let name = edit_tag_name();
                                        spawn(async move {
                                            if rename_tag(tid, name).await.is_ok() {
                                                edit_tag.set(None);
                                                tags.restart();
                                            }
                                        });
                                    };
                                    rsx! {
                                        span { key: "{tid}", class: "flex items-center gap-1 rounded-full border border-white/15 px-2 py-0.5 text-xs",
                                            if edit_tag() == Some(tid) {
                                                input {
                                                    class: "w-24 bg-transparent text-xs focus:outline-none",
                                                    value: "{edit_tag_name}",
                                                    oninput: move |e| edit_tag_name.set(e.value()),
                                                    onkeydown: move |e| if e.key() == Key::Enter { save(()) },
                                                }
                                                button { class: "text-brand-400", onclick: move |_| save(()), "✓" }
                                                button { class: "text-white/50", onclick: move |_| edit_tag.set(None), "✕" }
                                            } else {
                                                button { class: "hover:underline",
                                                    onclick: move |_| { edit_tag_name.set(tname.clone()); edit_tag.set(Some(tid)); },
                                                    "#{tname}"
                                                }
                                                button { class: "text-red-400",
                                                    onclick: move |_| { spawn(async move { if delete_tag(tid).await.is_ok() { tags.restart(); } }); },
                                                    "✕"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
