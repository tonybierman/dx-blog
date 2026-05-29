//! Public reader pages: post detail, category/tag/author feeds, archive,
//! search, and subscribe.

use dioxus::prelude::*;

use arium_dioxus::ui::{use_permissions, ResourceGate};
use arium_dioxus::ResourceRole;

use crate::layouts::{BentoGridLayout, FullBleedLayout, HolyGrailLayout, MasonryLayout};
use crate::pages::widgets::{
    CategoryList, FeedGrid, FeedSkeleton, PaginationBar, PostCardView, TagList,
};
use crate::server::authors::get_author_profile;
use crate::server::comments::{create_comment, list_comments};
use crate::server::posts::{get_post, list_archive, list_posts, posts_by_author};
use crate::server::search::search_posts;
use crate::server::subscribers::{confirm_subscription, subscribe};
use crate::server::taxonomy::{get_category, get_tag};
use crate::Route;

// ---------------------------------------------------------------- Post detail

#[component]
pub fn PostDetail(slug: String) -> Element {
    rsx! {
        FullBleedLayout {
            div { class: "mx-auto max-w-3xl px-4 py-10",
                Link { to: Route::HomePage, class: "text-sm text-white/50 hover:underline", "← Back" }
                // The post (and its <head> tags) load inside a suspense boundary so
                // the skeleton shows during client-side navigation, while the SSR
                // pass still waits for the data — see `PostContent`.
                SuspenseBoundary {
                    fallback: |_| rsx! { PostDetailSkeleton {} },
                    // Key on slug so navigating between posts remounts the subtree,
                    // re-firing the view-recording effect and resetting form state.
                    PostContent { key: "{slug}", slug }
                }
            }
        }
    }
}

/// Loads the post via `use_server_future` (not `use_resource`) so the resolved
/// article — and its Open Graph / `<head>` tags — are part of the server-rendered
/// HTML that crawlers and link-unfurlers read. `use_resource` would render the
/// skeleton during SSR, leaving those tags invisible to anything without JS.
#[component]
fn PostContent(slug: String) -> Element {
    let post = use_server_future(use_reactive!(|(slug,)| async move { get_post(slug).await }))?;
    let site = use_server_future(crate::server::settings::get_site_meta)?;

    let post = post.read();
    let post = post.as_ref().unwrap();
    let base_url = match &*site.read() {
        Some(Ok(m)) => m.base_url.clone(),
        _ => String::new(),
    };

    match post {
        Ok(Some(p)) => {
            let p = p.clone();
            rsx! {
                PostHead { post: p.clone(), base_url }
                PostBody { post: p }
            }
        }
        Ok(None) => rsx! { p { class: "mt-8 text-white/60", "Post not found." } },
        Err(e) => rsx! { p { class: "mt-8 text-red-400", "Error: {e}" } },
    }
}

/// Per-post `<head>`: title plus Open Graph / Twitter card tags. Open Graph
/// wants absolute URLs, so `og:url` and `og:image` are joined onto `base_url`
/// (the canonical origin from `SITE_URL`); already-absolute image URLs pass
/// through unchanged.
#[component]
fn PostHead(post: crate::model::PostDetail, base_url: String) -> Element {
    let url = format!("{base_url}/post/{}", post.slug);
    let image = post.featured_image_url.as_ref().map(|img| {
        if img.starts_with("http://") || img.starts_with("https://") {
            img.clone()
        } else if img.starts_with('/') {
            format!("{base_url}{img}")
        } else {
            format!("{base_url}/{img}")
        }
    });
    let description = post.excerpt.clone();

    rsx! {
        document::Title { "{post.title}" }
        document::Meta { name: "description", content: "{description}" }
        document::Meta { property: "og:type", content: "article" }
        document::Meta { property: "og:title", content: "{post.title}" }
        document::Meta { property: "og:description", content: "{description}" }
        document::Meta { property: "og:url", content: "{url}" }
        document::Meta { name: "twitter:title", content: "{post.title}" }
        document::Meta { name: "twitter:description", content: "{description}" }
        if let Some(img) = image {
            document::Meta { property: "og:image", content: "{img}" }
            document::Meta { name: "twitter:image", content: "{img}" }
        }
        // Normalize the stored SQLite datetime ("YYYY-MM-DD HH:MM:SS", UTC) to
        // the ISO 8601 form Open Graph expects.
        if let Some(when) = post.published_at.as_ref().map(|w| {
            let t = w.replace(' ', "T");
            if t.ends_with('Z') || t.contains('+') { t } else { format!("{t}Z") }
        }) {
            document::Meta { property: "article:published_time", content: "{when}" }
        }
        document::Meta { property: "article:author", content: "{post.author_name}" }
    }
}

/// Article-shaped placeholder shown while a single post loads.
#[component]
fn PostDetailSkeleton() -> Element {
    use arium_dioxus::ui::components::skeleton::Skeleton;
    rsx! {
        div { class: "mt-8 space-y-4",
            Skeleton { style: "height: 16rem; width: 100%; border-radius: 0.75rem;" }
            Skeleton { style: "height: 2rem; width: 70%;" }
            Skeleton { style: "height: 1rem; width: 12rem;" }
            div { class: "mt-6 space-y-3",
                Skeleton { style: "height: 0.9rem; width: 100%;" }
                Skeleton { style: "height: 0.9rem; width: 95%;" }
                Skeleton { style: "height: 0.9rem; width: 90%;" }
                Skeleton { style: "height: 0.9rem; width: 97%;" }
            }
        }
    }
}

#[component]
fn PostBody(post: crate::model::PostDetail) -> Element {
    let post_id = post.id;
    // Record a view once the post is on screen.
    use_effect(move || {
        spawn(async move {
            let _ = crate::server::analytics::record_view(post_id, None).await;
        });
    });

    let is_draft = post.status != "published";

    rsx! {
        article {
            if is_draft {
                div { class: "mb-6 rounded-lg border border-amber-400/30 bg-amber-400/10 px-4 py-2 text-sm text-amber-200",
                    "Draft preview — this post is not published and is only visible to you."
                }
            }
            if let Some(img) = post.featured_image_url.clone() {
                img { class: "mb-6 max-h-96 w-full rounded-xl object-cover", src: "{img}", alt: "{post.title}" }
            }
            div { class: "flex items-start justify-between gap-4",
                h1 { class: "text-3xl font-bold", "{post.title}" }
                // Editors/owners of this post (or global admins) get an inline
                // edit link straight to the admin editor.
                ResourceGate { kind: "post", id: post.id, min_role: ResourceRole::Editor,
                    Link {
                        to: Route::AdminPostEdit { id: post.id },
                        class: "shrink-0 rounded border border-white/15 px-2 py-1 text-sm text-white/70 hover:bg-white/5",
                        "Edit"
                    }
                }
            }
            div { class: "mt-2 flex gap-2 text-sm text-white/50",
                Link {
                    to: Route::AuthorProfile { slug: post.author_username.clone() },
                    class: "hover:underline",
                    "{post.author_name}"
                }
                if let Some(when) = post.published_at.clone() {
                    span { "· {when}" }
                }
            }
            div { class: "prose mt-8 max-w-none", dangerous_inner_html: "{post.body_html}" }

            // Author bio
            if let Some(bio) = post.author_bio.clone() {
                div { class: "mt-10 rounded-xl border border-white/10 bg-white/[0.03] p-4",
                    h3 { class: "font-semibold", "About {post.author_name}" }
                    p { class: "mt-1 text-sm text-white/60", "{bio}" }
                }
            }
        }
        CommentSection { post_id }
    }
}

#[component]
fn CommentSection(post_id: i64) -> Element {
    let perms = use_permissions();
    let logged_in = perms.is_authenticated();
    let mut comments = use_resource(move || async move { list_comments(post_id).await });

    let mut body = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut email = use_signal(String::new);
    let mut status = use_signal(String::new);

    let submit = move |_| {
        let b = body();
        let (n, e) = (name(), email());
        spawn(async move {
            let gname = if n.is_empty() { None } else { Some(n) };
            let gemail = if e.is_empty() { None } else { Some(e) };
            match create_comment(post_id, b, gname, gemail).await {
                Ok(()) => {
                    body.set(String::new());
                    status.set("Thanks! Your comment is awaiting approval.".into());
                    comments.restart();
                }
                Err(err) => status.set(arium_dioxus::friendly_server_error(err)),
            }
        });
    };

    rsx! {
        section { class: "mt-12 border-t border-white/10 pt-8",
            h2 { class: "text-xl font-semibold", "Comments" }
            div { class: "mt-4 space-y-4",
                match &*comments.read() {
                    Some(Ok(list)) if !list.is_empty() => rsx! {
                        for c in list.clone() {
                            div { key: "{c.id}", class: "rounded-lg border border-white/10 p-3",
                                div { class: "text-sm font-medium", "{c.display_name}" }
                                div { class: "text-xs text-white/40", "{c.created_at}" }
                                p { class: "mt-1 text-sm text-white/80", "{c.body}" }
                            }
                        }
                    },
                    Some(Ok(_)) => rsx! { p { class: "text-sm text-white/50", "No comments yet." } },
                    Some(Err(e)) => rsx! { p { class: "text-sm text-red-400", "{e}" } },
                    None => rsx! { p { class: "text-sm text-white/50", "Loading…" } },
                }
            }

            div { class: "mt-6 space-y-2",
                h3 { class: "font-medium", "Leave a comment" }
                if !logged_in {
                    div { class: "flex gap-2",
                        input {
                            class: "w-1/2 rounded border border-white/15 bg-transparent px-2 py-1 text-sm",
                            placeholder: "Name",
                            value: "{name}",
                            oninput: move |e| name.set(e.value()),
                        }
                        input {
                            class: "w-1/2 rounded border border-white/15 bg-transparent px-2 py-1 text-sm",
                            placeholder: "Email",
                            value: "{email}",
                            oninput: move |e| email.set(e.value()),
                        }
                    }
                }
                textarea {
                    class: "h-24 w-full rounded border border-white/15 bg-transparent px-2 py-1 text-sm",
                    placeholder: "Your comment…",
                    value: "{body}",
                    oninput: move |e| body.set(e.value()),
                }
                button {
                    class: "rounded bg-brand-600 px-4 py-1.5 text-sm font-medium hover:bg-brand-500",
                    onclick: submit,
                    "Post comment"
                }
                if !status().is_empty() {
                    p { class: "text-sm text-white/60", "{status}" }
                }
            }
        }
    }
}

// ---------------------------------------------------------------- Feeds

/// Shared paginated feed body used by home/category feeds.
#[component]
fn PaginatedFeed(category_slug: Option<String>, tag_slug: Option<String>) -> Element {
    let mut page = use_signal(|| 1i64);
    let posts = use_resource(use_reactive!(|(category_slug, tag_slug)| async move {
        list_posts(page(), category_slug, tag_slug).await
    }));

    rsx! {
        match &*posts.read() {
            Some(Ok(feed)) => {
                let cards = feed.items.clone();
                let total_pages = feed.total_pages();
                rsx! {
                    FeedGrid { cards }
                    PaginationBar { page: page(), total_pages, on_change: move |p| page.set(p) }
                }
            }
            Some(Err(e)) => rsx! { p { class: "text-red-400", "Error: {e}" } },
            None => rsx! { FeedSkeleton {} },
        }
    }
}

#[component]
pub fn CategoryFeed(slug: String) -> Element {
    let cat = {
        let slug = slug.clone();
        use_resource(use_reactive!(
            |(slug,)| async move { get_category(slug).await }
        ))
    };
    let title = match &*cat.read() {
        Some(Ok(Some(c))) => c.name.clone(),
        _ => slug.clone(),
    };

    rsx! {
        HolyGrailLayout {
            left: rsx! { CategoryList {} TagList {} },
            h1 { class: "mb-6 text-2xl font-bold", "Category: {title}" }
            PaginatedFeed { category_slug: Some(slug.clone()), tag_slug: None }
        }
    }
}

#[component]
pub fn TagFeed(slug: String) -> Element {
    let tag = {
        let slug = slug.clone();
        use_resource(use_reactive!(|(slug,)| async move { get_tag(slug).await }))
    };
    let title = match &*tag.read() {
        Some(Ok(Some(t))) => t.name.clone(),
        _ => slug.clone(),
    };
    let mut page = use_signal(|| 1i64);
    let posts = {
        let slug = slug.clone();
        use_resource(use_reactive!(|(slug,)| async move {
            list_posts(page(), None, Some(slug)).await
        }))
    };

    rsx! {
        BentoGridLayout {
            left: rsx! {
                h1 { class: "text-2xl font-bold", "#{title}" }
                TagList {}
            },
            match &*posts.read() {
                Some(Ok(feed)) => {
                    let total_pages = feed.total_pages();
                    rsx! {
                        for (i, card) in feed.items.clone().into_iter().enumerate() {
                            div {
                                key: "{card.id}",
                                class: if i == 0 { "col-span-2 row-span-2" } else { "" },
                                PostCardView { card }
                            }
                        }
                        // Span the full bento row so the pager sits below the tiles.
                        div { style: "grid-column: 1 / -1;",
                            PaginationBar { page: page(), total_pages, on_change: move |p| page.set(p) }
                        }
                    }
                }
                Some(Err(e)) => rsx! { p { class: "text-red-400", "Error: {e}" } },
                None => rsx! { FeedSkeleton {} },
            }
        }
    }
}

#[component]
pub fn AuthorProfile(slug: String) -> Element {
    let profile = {
        let slug = slug.clone();
        use_resource(use_reactive!(|(slug,)| async move {
            get_author_profile(slug).await
        }))
    };
    let mut page = use_signal(|| 1i64);
    let posts = {
        let slug = slug.clone();
        use_resource(use_reactive!(|(slug,)| async move {
            posts_by_author(slug, page()).await
        }))
    };

    let sidebar = match &*profile.read() {
        Some(Ok(Some(p))) => {
            let p = p.clone();
            rsx! {
                div {
                    if let Some(av) = p.avatar_url.clone() {
                        img { class: "mb-3 h-20 w-20 rounded-full object-cover", src: "{av}" }
                    }
                    h2 { class: "text-lg font-semibold", "{p.display_name}" }
                    p { class: "text-sm text-white/40", "@{p.username}" }
                    if let Some(bio) = p.bio.clone() {
                        p { class: "mt-2 text-sm text-white/60", "{bio}" }
                    }
                }
            }
        }
        _ => rsx! { p { class: "text-white/40", "Author" } },
    };

    rsx! {
        HolyGrailLayout {
            left: sidebar,
            h1 { class: "mb-6 text-2xl font-bold", "Posts" }
            match &*posts.read() {
                Some(Ok(feed)) => {
                    let cards = feed.items.clone();
                    let total_pages = feed.total_pages();
                    rsx! {
                        FeedGrid { cards }
                        PaginationBar { page: page(), total_pages, on_change: move |p| page.set(p) }
                    }
                }
                Some(Err(e)) => rsx! { p { class: "text-red-400", "Error: {e}" } },
                None => rsx! { FeedSkeleton {} },
            }
        }
    }
}

#[component]
pub fn Archive() -> Element {
    let mut page = use_signal(|| 1i64);
    let posts = use_resource(move || async move { list_archive(page()).await });
    rsx! {
        MasonryLayout {
            h1 { class: "mb-6 text-2xl font-bold", "Archive" }
            match &*posts.read() {
                Some(Ok(feed)) => {
                    let total_pages = feed.total_pages();
                    rsx! {
                        for card in feed.items.clone() {
                            div { key: "{card.id}", class: "mb-4 inline-block w-full break-inside-avoid",
                                PostCardView { card }
                            }
                        }
                        // `column-span: all` lifts the pager out of the
                        // CSS-columns flow so it spans the full masonry width.
                        div { style: "column-span: all;",
                            PaginationBar { page: page(), total_pages, on_change: move |p| page.set(p) }
                        }
                    }
                }
                Some(Err(e)) => rsx! { p { class: "text-red-400", "Error: {e}" } },
                None => rsx! { FeedSkeleton {} },
            }
        }
    }
}

#[component]
pub fn SearchResults(q: String) -> Element {
    let mut query = use_signal(|| q.clone());
    let mut page = use_signal(|| 1i64);
    // Facet state — category/tag slugs ("" = any) and a date bucket.
    let mut category = use_signal(String::new);
    let mut tag = use_signal(String::new);
    let mut date_range = use_signal(String::new);

    let cats = use_resource(crate::server::taxonomy::list_categories);
    let tags = use_resource(crate::server::taxonomy::list_tags);

    let results = use_resource(move || {
        let q = query();
        let (c, t, d) = (category(), tag(), date_range());
        async move {
            let c = if c.is_empty() { None } else { Some(c) };
            let t = if t.is_empty() { None } else { Some(t) };
            let d = if d.is_empty() { None } else { Some(d) };
            search_posts(q, page(), c, t, d).await
        }
    });

    rsx! {
        HolyGrailLayout {
            right: rsx! {
                div { class: "space-y-5 text-sm",
                    h3 { class: "font-semibold text-white/80", "Refine" }
                    // Category facet
                    div {
                        label { class: "mb-1 block text-xs uppercase tracking-wide text-white/40", "Category" }
                        select {
                            class: "w-full rounded border border-white/15 bg-transparent px-2 py-1.5",
                            onchange: move |e| { category.set(e.value()); page.set(1); },
                            option { value: "", selected: category().is_empty(), "All categories" }
                            if let Some(Ok(list)) = &*cats.read() {
                                for c in list.clone() {
                                    option { value: "{c.slug}", selected: category() == c.slug, "{c.name}" }
                                }
                            }
                        }
                    }
                    // Tag facet
                    div {
                        label { class: "mb-1 block text-xs uppercase tracking-wide text-white/40", "Tag" }
                        select {
                            class: "w-full rounded border border-white/15 bg-transparent px-2 py-1.5",
                            onchange: move |e| { tag.set(e.value()); page.set(1); },
                            option { value: "", selected: tag().is_empty(), "All tags" }
                            if let Some(Ok(list)) = &*tags.read() {
                                for t in list.clone() {
                                    option { value: "{t.slug}", selected: tag() == t.slug, "#{t.name}" }
                                }
                            }
                        }
                    }
                    // Date facet
                    div {
                        label { class: "mb-1 block text-xs uppercase tracking-wide text-white/40", "Published" }
                        select {
                            class: "w-full rounded border border-white/15 bg-transparent px-2 py-1.5",
                            onchange: move |e| { date_range.set(e.value()); page.set(1); },
                            option { value: "", selected: date_range().is_empty(), "Any time" }
                            option { value: "week", selected: date_range() == "week", "Past week" }
                            option { value: "month", selected: date_range() == "month", "Past month" }
                            option { value: "year", selected: date_range() == "year", "Past year" }
                        }
                    }
                    if !category().is_empty() || !tag().is_empty() || !date_range().is_empty() {
                        button {
                            class: "text-xs text-brand-400 hover:underline",
                            onclick: move |_| { category.set(String::new()); tag.set(String::new()); date_range.set(String::new()); page.set(1); },
                            "Clear filters"
                        }
                    }
                }
            },
            h1 { class: "mb-4 text-2xl font-bold", "Search" }
            input {
                class: "mb-6 w-full rounded border border-white/15 bg-transparent px-3 py-2",
                placeholder: "Search posts…",
                value: "{query}",
                oninput: move |e| { query.set(e.value()); page.set(1); },
            }
            match &*results.read() {
                Some(Ok(feed)) => {
                    let cards = feed.items.clone();
                    let total_pages = feed.total_pages();
                    rsx! {
                        p { class: "mb-4 text-sm text-white/50", "{feed.total} result(s)" }
                        FeedGrid { cards }
                        PaginationBar { page: page(), total_pages, on_change: move |p| page.set(p) }
                    }
                }
                Some(Err(e)) => rsx! { p { class: "text-red-400", "Error: {e}" } },
                None => rsx! { FeedSkeleton {} },
            }
        }
    }
}

#[component]
pub fn Subscribe() -> Element {
    let mut email = use_signal(String::new);
    let mut status = use_signal(String::new);

    let submit = move |_| {
        let e = email();
        spawn(async move {
            match subscribe(e).await {
                Ok(()) => {
                    email.set(String::new());
                    status.set(
                        "Almost there — check your inbox to confirm your subscription.".into(),
                    );
                }
                Err(err) => status.set(arium_dioxus::friendly_server_error(err)),
            }
        });
    };

    rsx! {
        FullBleedLayout {
            div { class: "flex min-h-screen flex-col items-center justify-center gap-4 p-4 text-center",
                h1 { class: "text-3xl font-bold", "Subscribe" }
                p { class: "max-w-md text-white/60", "Get new posts in your inbox. No spam." }
                div { class: "flex w-full max-w-md gap-2",
                    input {
                        class: "flex-1 rounded border border-white/15 bg-transparent px-3 py-2",
                        r#type: "email",
                        placeholder: "you@example.com",
                        value: "{email}",
                        oninput: move |e| email.set(e.value()),
                    }
                    button {
                        class: "rounded bg-brand-600 px-4 py-2 font-medium hover:bg-brand-500",
                        onclick: submit,
                        "Subscribe"
                    }
                }
                if !status().is_empty() {
                    p { class: "text-sm text-white/70", "{status}" }
                }
                Link { to: Route::HomePage, class: "text-sm text-white/50 hover:underline", "← Home" }
            }
        }
    }
}

/// Landing page for the confirmation link in the double opt-in email. Consumes
/// the token on mount and reports whether the subscription was confirmed.
#[component]
pub fn ConfirmSubscription(token: String) -> Element {
    let outcome = use_resource(use_reactive!(|(token,)| async move {
        confirm_subscription(token).await
    }));

    rsx! {
        FullBleedLayout {
            div { class: "flex min-h-screen flex-col items-center justify-center gap-4 p-4 text-center",
                match &*outcome.read() {
                    Some(Ok(true)) => rsx! {
                        h1 { class: "text-3xl font-bold", "Subscription confirmed 🎉" }
                        p { class: "max-w-md text-white/60", "Thanks — you'll now receive new posts in your inbox." }
                    },
                    Some(Ok(false)) => rsx! {
                        h1 { class: "text-3xl font-bold", "Link expired" }
                        p { class: "max-w-md text-white/60",
                            "This confirmation link is invalid or has already been used. Try subscribing again."
                        }
                        Link { to: Route::Subscribe, class: "text-sm text-brand-400 hover:underline", "Subscribe →" }
                    },
                    Some(Err(e)) => rsx! { p { class: "text-red-400", "Error: {e}" } },
                    None => rsx! { p { class: "text-white/50", "Confirming…" } },
                }
                Link { to: Route::HomePage, class: "text-sm text-white/50 hover:underline", "← Home" }
            }
        }
    }
}
