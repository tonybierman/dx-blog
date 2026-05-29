//! Public reader pages: post detail, category/tag/author feeds, archive,
//! search, and subscribe.

use dioxus::prelude::*;

use arium_dioxus::ui::{use_permissions, ResourceGate};
use arium_dioxus::ResourceRole;
use dioxus_sdk_time::use_interval;

use crate::layouts::{BentoGridLayout, FullBleedLayout, HolyGrailLayout, MasonryLayout};
use crate::live::{use_live, LiveHandle};
use crate::model::CommentView;
use crate::pages::widgets::{CategoryList, FeedSection, FeedShape, TagList};
use crate::server::authors::get_author_profile;
use crate::server::comments::{create_comment, list_comments};
use crate::server::posts::{get_post, list_archive, list_posts, posts_by_author};
use crate::server::reactions::{add_reaction, reaction_total};
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
        document::Meta { property: "og:type", content: "article" }
        document::Meta { property: "og:title", content: "{post.title}" }
        document::Meta { name: "twitter:title", content: "{post.title}" }
        // Guard the description/url tags: a post may have no excerpt, and `url`
        // is only meaningful when SITE_URL resolved (else `base_url` is empty and
        // `og:url` would be a bare relative path). Same treatment as `og:image`.
        if !description.is_empty() {
            document::Meta { name: "description", content: "{description}" }
            document::Meta { property: "og:description", content: "{description}" }
            document::Meta { name: "twitter:description", content: "{description}" }
        }
        if !base_url.is_empty() {
            document::Meta { property: "og:url", content: "{url}" }
        }
        if let Some(img) = image {
            document::Meta { property: "og:image", content: "{img}" }
            document::Meta { name: "twitter:image", content: "{img}" }
        }
        // Normalize the stored SQLite datetime ("YYYY-MM-DD HH:MM:SS", UTC) to
        // the ISO 8601 form Open Graph expects (shared with the feed/sitemap).
        if let Some(when) = post.published_at.as_ref().map(|w| crate::model::to_rfc3339(w)) {
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

    // Open the live channel for this post once; share the handle with the
    // presence badge, the reaction bar, and the comment section below.
    let live = use_live(post_id);

    let is_draft = post.status != crate::model::STATUS_PUBLISHED;

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
            div { class: "mt-2 flex flex-wrap items-center gap-2 text-sm text-white/50",
                Link {
                    to: Route::AuthorProfile { slug: post.author_username.clone() },
                    class: "hover:underline",
                    "{post.author_name}"
                }
                if let Some(when) = post.published_at.clone() {
                    span { "· {when}" }
                }
                PresenceBadge { live }
            }
            // "Rust MDX": split the body into rendered-markdown runs and live
            // embed blocks, mounting each embed as a real interactive component
            // interleaved with the prose (see `crate::mdx`).
            div { class: "prose mt-8 max-w-none",
                for (i, seg) in crate::mdx::parse_body(&post.body_md).into_iter().enumerate() {
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

            // Author bio
            if let Some(bio) = post.author_bio.clone() {
                div { class: "mt-10 rounded-xl border border-white/10 bg-white/[0.03] p-4",
                    h3 { class: "font-semibold", "About {post.author_name}" }
                    p { class: "mt-1 text-sm text-white/60", "{bio}" }
                }
            }
        }
        ReactionBar { post_id, live }
        CommentSection { post_id, live }
    }
}

/// "N reading now" — the live presence count for this post. Count is 0 during
/// SSR and until the client's EventSource connects, so it's hidden then; once
/// connected it shows at least "1 reading now" (you). Note readers are deduped
/// by a coarse IP+User-Agent fingerprint, so multiple tabs/windows of the same
/// browser on one machine count as a single reader — a second browser shows 2.
#[component]
fn PresenceBadge(live: LiveHandle) -> Element {
    let n = (live.reading_now)();
    rsx! {
        if n >= 1 {
            span { class: "inline-flex items-center gap-1 rounded-full bg-emerald-400/10 px-2 py-0.5 text-xs text-emerald-300",
                span { class: "h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-400" }
                "{n} reading now"
            }
        }
    }
}

/// A clap button plus the floating-clap overlay. Clapping is anonymous and
/// optimistic: each click animates a burst locally right away and fires
/// `add_reaction`; every reader (including this one, via the SSE echo) gets the
/// broadcast burst too. The count is the server's authoritative total: it seeds
/// from `reaction_total` on load and each reaction event carries the new total,
/// so every window converges on the same number.
#[component]
fn ReactionBar(post_id: i64, live: LiveHandle) -> Element {
    let mut claps = live.claps;
    let total = use_resource(move || async move { reaction_total(post_id).await });
    let mut local_clap_id = use_signal(|| 1_000_000_000u64);

    // Prune finished animations (~1.4s keyframe) so the overlay vec stays small.
    use_interval(std::time::Duration::from_millis(1500), move |()| {
        if !claps().is_empty() {
            claps.set(Vec::new());
        }
    });

    // The initial fetch and the live total reflect the same DB count; the live
    // total (once any event has arrived) is always >= the fetch, so max() shows
    // the right number in any arrival order.
    let base = match &*total.read() {
        Some(Ok(n)) => *n,
        _ => 0,
    };
    let display = base.max((live.reaction_count)());

    let clap = move |_| {
        // Optimistic local burst for instant feedback; the server echo adds the
        // shared one. Use a high id range so it can't collide with server bursts.
        let id = local_clap_id();
        local_clap_id += 1;
        claps.with_mut(|v| {
            v.push(crate::live::ClapBurst {
                id,
                kind: "clap".into(),
            })
        });
        spawn(async move {
            let _ = add_reaction(post_id, "clap".to_string()).await;
        });
    };

    rsx! {
        div { class: "relative mt-10 flex items-center gap-3",
            button {
                class: "inline-flex items-center gap-2 rounded-full border border-white/15 px-4 py-1.5 text-sm hover:bg-white/5 active:scale-95",
                onclick: clap,
                span { "👏" }
                "Clap"
            }
            span { class: "text-sm text-white/50", "{display}" }
            // Floating bursts rise out of the button row.
            div { class: "pointer-events-none absolute bottom-0 left-3",
                for burst in claps() {
                    span { key: "{burst.id}", class: "clap-float", "👏" }
                }
            }
        }
    }
}

#[component]
fn CommentSection(post_id: i64, live: LiveHandle) -> Element {
    let perms = use_permissions();
    let logged_in = perms.is_authenticated();
    let comments = use_resource(move || async move { list_comments(post_id).await });

    let mut body = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut email = use_signal(String::new);
    let mut status = use_signal(String::new);
    // Guards against a double-submit while the request is in flight.
    let mut submitting = use_signal(|| false);
    // Locally-held comments: optimistic placeholders (negative ids) while a post
    // is in flight, and our own pending comments (real ids) that won't arrive
    // over SSE because only approved comments are broadcast.
    let mut optimistic = use_signal(Vec::<CommentView>::new);
    let mut next_temp_id = use_signal(|| -1i64);

    let mut live_comments = live.live_comments;

    let submit = move |_| {
        if submitting() {
            return;
        }
        let b = body().trim().to_string();
        if b.is_empty() {
            status.set("Comment cannot be empty.".into());
            return;
        }
        let (n, e) = (name(), email());
        submitting.set(true);

        // Drop an optimistic placeholder in immediately so the comment appears
        // the instant you hit post; reconcile against the server's return value.
        let temp_id = next_temp_id();
        next_temp_id -= 1;
        let display = if n.trim().is_empty() {
            "You".to_string()
        } else {
            n.trim().to_string()
        };
        optimistic.with_mut(|v| {
            v.push(CommentView {
                id: temp_id,
                post_id,
                display_name: display,
                body: b.clone(),
                status: "sending".to_string(),
                created_at: "just now".to_string(),
            })
        });

        spawn(async move {
            let gname = if n.is_empty() { None } else { Some(n) };
            let gemail = if e.is_empty() { None } else { Some(e) };
            match create_comment(post_id, b, gname, gemail).await {
                Ok(view) => {
                    // Clear the whole form, not just the body, so a guest doesn't
                    // resubmit their name/email by accident.
                    body.set(String::new());
                    name.set(String::new());
                    email.set(String::new());
                    // Replace the placeholder with the server's canonical row.
                    if view.status == "approved" {
                        // Approved: it also streams in via SSE — add it deduped so
                        // it shows even if the echo is slow, then drop the temp.
                        optimistic.with_mut(|v| v.retain(|c| c.id != temp_id));
                        live_comments.with_mut(|v| {
                            if !v.iter().any(|c| c.id == view.id) {
                                v.push(view.clone());
                            }
                        });
                        status.set("Thanks! Your comment is posted.".into());
                    } else {
                        // Pending: never broadcast, so keep showing it locally,
                        // marked awaiting approval (swap temp → real row).
                        optimistic.with_mut(|v| {
                            for c in v.iter_mut() {
                                if c.id == temp_id {
                                    *c = view.clone();
                                }
                            }
                        });
                        status.set("Thanks! Your comment is awaiting approval.".into());
                    }
                }
                Err(err) => {
                    optimistic.with_mut(|v| v.retain(|c| c.id != temp_id));
                    status.set(arium_dioxus::friendly_server_error(err));
                }
            }
            submitting.set(false);
        });
    };

    // Merge the three sources into one list, deduped by id and preserving order:
    // initial approved set (oldest first), then SSE-streamed approved comments,
    // then our local optimistic/pending rows.
    let merged: Vec<CommentView> = {
        let mut out: Vec<CommentView> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        if let Some(Ok(list)) = &*comments.read() {
            for c in list {
                if seen.insert(c.id) {
                    out.push(c.clone());
                }
            }
        }
        for c in live_comments() {
            if c.post_id == post_id && seen.insert(c.id) {
                out.push(c);
            }
        }
        for c in optimistic() {
            if seen.insert(c.id) {
                out.push(c);
            }
        }
        out
    };
    let loading = comments.read().is_none();
    let load_error = matches!(&*comments.read(), Some(Err(_)));

    rsx! {
        section { class: "mt-12 border-t border-white/10 pt-8",
            h2 { class: "text-xl font-semibold", "Comments" }
            div { class: "mt-4 space-y-4",
                if !merged.is_empty() {
                    for c in merged {
                        div { key: "{c.id}", class: "rounded-lg border border-white/10 p-3",
                            div { class: "flex items-center gap-2",
                                div { class: "text-sm font-medium", "{c.display_name}" }
                                if c.status == "sending" {
                                    span { class: "text-xs text-white/40 italic", "posting…" }
                                } else if c.status == "pending" {
                                    span { class: "rounded bg-amber-400/10 px-1.5 text-xs text-amber-300", "awaiting approval" }
                                }
                            }
                            div { class: "text-xs text-white/40", "{c.created_at}" }
                            p { class: "mt-1 text-sm text-white/80", "{c.body}" }
                        }
                    }
                } else if loading {
                    p { class: "text-sm text-white/50", "Loading…" }
                } else if load_error {
                    p { class: "text-sm text-red-400", "Couldn't load comments." }
                } else {
                    p { class: "text-sm text-white/50", "No comments yet." }
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
                    class: "rounded bg-brand-600 px-4 py-1.5 text-sm font-medium hover:bg-brand-500 disabled:opacity-50",
                    disabled: submitting(),
                    onclick: submit,
                    if submitting() { "Posting…" } else { "Post comment" }
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
    // This component is reused (not remounted) when the route param changes, so
    // the page signal would otherwise carry over — land on page 3 of one feed,
    // switch feeds, and you'd be stuck on a page 3 that may not exist. Reset to 1
    // whenever the filter changes.
    use_effect({
        let (category_slug, tag_slug) = (category_slug.clone(), tag_slug.clone());
        use_reactive!(|(category_slug, tag_slug)| {
            let _ = (&category_slug, &tag_slug);
            page.set(1);
        })
    });
    let posts = use_resource(use_reactive!(|(category_slug, tag_slug)| async move {
        list_posts(page(), category_slug, tag_slug).await
    }));

    rsx! { FeedSection { posts, shape: FeedShape::Grid, page } }
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
    // Reset pagination when the tag changes (the component is reused across
    // route-param changes, so the page signal would otherwise persist).
    use_effect({
        let slug = slug.clone();
        use_reactive!(|(slug,)| {
            let _ = slug;
            page.set(1);
        })
    });
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
            FeedSection { posts, shape: FeedShape::Bento, page }
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
    // Reset pagination when navigating to a different author (reused component).
    use_effect({
        let slug = slug.clone();
        use_reactive!(|(slug,)| {
            let _ = slug;
            page.set(1);
        })
    });
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
            FeedSection { posts, shape: FeedShape::Grid, page }
        }
    }
}

#[component]
pub fn Archive() -> Element {
    let page = use_signal(|| 1i64);
    let posts = use_resource(move || async move { list_archive(page()).await });
    rsx! {
        MasonryLayout {
            h1 { class: "mb-6 text-2xl font-bold", "Archive" }
            FeedSection { posts, shape: FeedShape::Masonry, page }
        }
    }
}

#[component]
pub fn SearchResults(q: String) -> Element {
    let mut query = use_signal(|| q.clone());
    let mut page = use_signal(|| 1i64);
    // The query signal seeds from `?q=` at mount only; this page is reused when
    // the route param changes (e.g. a second search from the header), so without
    // this the input would keep showing the old term. Re-sync on every change and
    // reset pagination.
    use_effect(use_reactive!(|(q,)| {
        query.set(q);
        page.set(1);
    }));
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
            if let Some(Ok(feed)) = &*results.read() {
                p { class: "mb-4 text-sm text-white/50", "{feed.total} result(s)" }
            }
            FeedSection { posts: results, shape: FeedShape::Grid, page }
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
