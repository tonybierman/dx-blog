//! Admin & authoring surface. The server fns are the real authorization
//! boundary; `RequirePermission` here just keeps unauthorized users out of the
//! UI and redirects them to sign in.

use dioxus::prelude::*;
use dioxus_sdk_time::use_debounce;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use arium_dioxus::ui::{Policy, RequirePermission};

use crate::auth_tokens::ADMIN_NAV_TOKENS;
use crate::model::{AnalyticsSummary, HomeLayout, PostEditData, POST_STATUSES, STATUS_DRAFT};
use crate::pages::widgets::list_states;
use crate::server::admin::*;
use crate::server::analytics::{analytics_summary, top_posts, top_referrers, views_over_time};
use crate::server::settings::{
    get_home_layout, get_site_tagline, get_site_title, get_theme_hue, set_home_layout,
    set_site_tagline, set_site_title, set_theme_hue, DEFAULT_THEME_HUE,
};
use crate::server::taxonomy::{list_categories, list_tags};
use crate::Route;

fn admin_any_policy() -> Policy {
    Policy::any_of(ADMIN_NAV_TOKENS)
}

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
                        Link { to: Route::AdminAppearance, class: nav_class(&active, "appearance"), "Appearance" }
                        Link { to: Route::AdminTaxonomy, class: nav_class(&active, "taxonomy"), "Taxonomy" }
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

/// The future an [`ActionButton`]'s `action` resolves to. Boxed so a single prop
/// type can carry any server-fn call.
type ActionFuture = Pin<Box<dyn Future<Output = Result<()>>>>;

/// One button for a fire-and-refetch admin mutation. Collapses the repeated
/// `spawn → server_fn → refetch` boilerplate (and its silent error swallowing)
/// that lived in five near-identical button components and two inline closures:
/// it runs `action`, calls `on_done` on success, shows the error inline on
/// failure, and blocks a double-click while the request is in flight.
#[component]
fn ActionButton(
    label: String,
    #[props(default = "text-brand-400 hover:underline".to_string())] class: String,
    on_done: EventHandler<()>,
    action: Callback<(), ActionFuture>,
) -> Element {
    let mut busy = use_signal(|| false);
    let mut err = use_signal(String::new);
    rsx! {
        button {
            class: "{class} disabled:opacity-50",
            disabled: busy(),
            onclick: move |_| {
                if busy() {
                    return;
                }
                busy.set(true);
                err.set(String::new());
                spawn(async move {
                    match action.call(()).await {
                        Ok(()) => on_done.call(()),
                        Err(e) => err.set(arium_dioxus::friendly_server_error(e)),
                    }
                    busy.set(false);
                });
            },
            "{label}"
        }
        if !err().is_empty() {
            span { class: "ml-2 text-xs text-red-400", "{err}" }
        }
    }
}

// ---------------------------------------------------------------- editor

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
                div { class: "prose max-w-none rounded border border-white/10 p-4",
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
            {list_states!(comments, empty: "No comments.", list => rsx! {
                        div { class: "space-y-3",
                            for c in list {
                                div { key: "{c.id}", class: "rounded-lg border border-white/10 p-3",
                                    div { class: "flex items-center justify-between",
                                        div { class: "text-sm font-medium", "{c.display_name}" }
                                        span { class: "rounded-full border border-white/15 px-2 py-0.5 text-xs", "{c.status}" }
                                    }
                                    p { class: "mt-1 text-sm text-white/80", "{c.body}" }
                                    div { class: "mt-2 flex gap-3 text-xs",
                                        {
                                            let cid = c.id;
                                            rsx! {
                                                ActionButton {
                                                    label: "Approve".to_string(),
                                                    on_done: move |_| comments.restart(),
                                                    action: move |_| Box::pin(async move { moderate_comment(cid, "approved".to_string()).await }) as ActionFuture,
                                                }
                                                ActionButton {
                                                    label: "Reject".to_string(),
                                                    on_done: move |_| comments.restart(),
                                                    action: move |_| Box::pin(async move { moderate_comment(cid, "rejected".to_string()).await }) as ActionFuture,
                                                }
                                                ActionButton {
                                                    label: "Delete".to_string(),
                                                    class: "text-red-400 hover:underline".to_string(),
                                                    on_done: move |_| comments.restart(),
                                                    action: move |_| Box::pin(async move { delete_comment(cid).await }) as ActionFuture,
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

// ---------------------------------------------------------------- settings (theme)

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
            h2 { class: "mb-1 text-lg font-semibold", "Theme" }
            p { class: "mb-3 text-sm text-white/50",
                "Pick the site accent. Changes preview instantly and save site-wide."
            }
            div { class: "flex flex-wrap gap-2",
                for (name, h) in presets {
                    button {
                        key: "{name}",
                        r#type: "button",
                        class: if hue() == h {
                            "flex items-center gap-1.5 rounded-full border border-white/50 px-3 py-1 text-xs"
                        } else {
                            "flex items-center gap-1.5 rounded-full border border-white/15 px-3 py-1 text-xs hover:border-white/40"
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
            h2 { class: "mb-1 text-lg font-semibold", "Home layout" }
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
        "flex h-16 w-24 flex-col gap-0.5 overflow-hidden rounded border border-white/10 bg-black/30 p-1";

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
                div { class: "absolute inset-y-1 left-1 w-3 rounded-sm bg-white/40" }
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

// ---------------------------------------------------------------- settings (site)

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
                    input {
                        class: "w-full rounded border border-white/15 bg-transparent px-3 py-2 text-sm",
                        placeholder: "dx-blog",
                        value: "{title_draft}",
                        oninput: move |e| { title_draft.set(e.value()); saved.set(false); err.set(String::new()); },
                    }
                    p { class: "mt-1 text-xs text-white/40", "Shown as the brand in the header and footer." }
                }
                div {
                    label { class: "mb-1 block text-sm font-medium", "Site tagline" }
                    input {
                        class: "w-full rounded border border-white/15 bg-transparent px-3 py-2 text-sm",
                        placeholder: "A short subtitle for your site",
                        value: "{tagline_draft}",
                        oninput: move |e| { tagline_draft.set(e.value()); saved.set(false); err.set(String::new()); },
                    }
                    p { class: "mt-1 text-xs text-white/40", "Shown beside the title in the header." }
                }
                div { class: "flex items-center gap-3",
                    button {
                        class: "rounded bg-brand-600 px-4 py-2 text-sm font-medium hover:bg-brand-500",
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

// ---------------------------------------------------------------- settings (appearance)

/// Visual settings: the accent theme and the public home-page layout.
#[component]
pub fn AdminAppearance() -> Element {
    rsx! {
        AdminShell { active: "appearance".to_string(),
            h1 { class: "mb-6 text-2xl font-bold", "Appearance" }
            ThemeSelector {}
            HomeLayoutSelector {}
        }
    }
}

// ---------------------------------------------------------------- settings (taxonomy)

#[component]
pub fn AdminTaxonomy() -> Element {
    rsx! {
        AdminShell { active: "taxonomy".to_string(),
            h1 { class: "mb-6 text-2xl font-bold", "Taxonomy" }
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
            h2 { class: "mb-3 text-lg font-semibold", "{kind.title()}" }
            div { class: "mb-3 flex gap-2",
                input {
                    class: "flex-1 rounded border border-white/15 bg-transparent px-2 py-1 text-sm",
                    placeholder: "{kind.placeholder()}",
                    value: "{new_name}",
                    oninput: move |e| new_name.set(e.value()),
                    onkeydown: move |e| if e.key() == Key::Enter { add(()) },
                }
                button { class: "rounded bg-brand-600 px-3 text-sm", onclick: move |_| add(()), "Add" }
            }
            if !err().is_empty() {
                p { class: "mb-2 text-xs text-red-400", "{err}" }
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
                                                input {
                                                    class: "flex-1 rounded border border-white/15 bg-transparent px-2 py-1 text-sm",
                                                    value: "{edit_name}",
                                                    oninput: move |e| edit_name.set(e.value()),
                                                    onkeydown: move |e| if e.key() == Key::Enter { save(()) },
                                                }
                                                button { class: "text-xs text-brand-400 hover:underline", onclick: move |_| save(()), "Save" }
                                                button { class: "text-xs text-white/50 hover:underline", onclick: move |_| edit_id.set(None), "Cancel" }
                                            } else {
                                                span { "{display}" }
                                                div { class: "flex gap-2",
                                                    button {
                                                        class: "text-xs text-white/60 hover:underline",
                                                        onclick: move |_| { edit_name.set(display.clone()); edit_id.set(Some(id)); },
                                                        "Edit"
                                                    }
                                                    button { class: "text-xs text-red-400 hover:underline", onclick: del, "Delete" }
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
                Some(Err(e)) => rsx! { p { class: "text-sm text-red-400", "{e}" } },
                None => rsx! { p { class: "text-sm text-white/50", "Loading…" } },
            }
        }
    }
}
