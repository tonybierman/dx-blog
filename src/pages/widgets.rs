//! Small shared UI pieces used across the reader pages.

use dioxus::prelude::*;

use arium_dioxus::ui::components::skeleton::Skeleton;

use crate::model::{PostCard, PostFeed};
use crate::server::comments::recent_comments;
use crate::server::posts::featured_posts;
use crate::server::taxonomy::{list_categories, list_tags};
use crate::Route;

/// Render the four states of a sidebar list `Resource<Result<Vec<_>>>` so each
/// sidebar only spells out its non-empty case. The loaded-but-empty / error /
/// pending arms — identical across every sidebar — live here once. The bound
/// `$list` is already `.clone()`d, so the body owns it and the read guard is
/// released before render.
macro_rules! sidebar_states {
    ($res:expr, empty: $empty:expr, $list:ident => $body:expr) => {
        match &*$res.read() {
            Some(Ok($list)) if !$list.is_empty() => {
                let $list = $list.clone();
                $body
            }
            Some(Ok(_)) => rsx! { p { class: "text-white/40", {$empty} } },
            Some(Err(_)) => rsx! { p { class: "text-white/40", "—" } },
            None => rsx! { p { class: "text-white/40", "…" } },
        }
    };
}

/// The loaded / error / loading arms of an admin list `Resource<Result<Vec<_>>>`.
/// Each call only spells out its non-empty `$list => $body` case and the empty
/// message; the error and "Loading…" arms — identical across the admin pages —
/// live here once. `$list` is `.clone()`d so the body owns it and the read guard
/// is released before render. (The richer-styled reader feeds use `feed_states!`.)
macro_rules! list_states {
    ($res:expr, empty: $empty:expr, $list:ident => $body:expr) => {{
        // Bound to a local so the `read()` guard's temporary is dropped at the
        // statement boundary rather than living to the end of the caller's block
        // (which would outlive the resource and fail to borrow-check at a tail
        // position).
        let states = match &*$res.read() {
            Some(Ok($list)) if !$list.is_empty() => {
                let $list = $list.clone();
                $body
            }
            Some(Ok(_)) => rsx! { p { class: "text-white/50", {$empty} } },
            Some(Err(e)) => rsx! { p { class: "text-red-400", "{e}" } },
            None => rsx! { p { class: "text-white/50", "Loading…" } },
        };
        states
    }};
}
pub(crate) use list_states;

/// The loaded / error / loading arms of a paginated post feed
/// `Resource<Result<PostFeed>>`. The loaded `PostFeed` is bound to `$feed` (by
/// reference) for the body to read `.items`/`.total`/`.total_pages()`; the error
/// arm shows an inline message and the loading arm a `FeedSkeleton`. Empty feeds
/// aren't special-cased — `FeedBody`/`FeedGrid` render the "No posts yet." note.
macro_rules! feed_states {
    ($res:expr, $feed:ident => $body:expr) => {{
        // See `list_states!` — bound to a local so the read guard drops before
        // the caller's block ends.
        let states = match &*$res.read() {
            Some(Ok($feed)) => $body,
            Some(Err(e)) => rsx! { p { class: "text-red-400", "Error: {e}" } },
            None => rsx! { FeedSkeleton {} },
        };
        states
    }};
}

/// How a feed's cards are arranged inside its layout. Most layouts render a
/// responsive [`FeedGrid`]; Bento and Masonry arrange the cards themselves and
/// need a full-width pager. Shared by the home page and the reader feeds so both
/// render through one [`FeedBody`].
#[derive(Clone, Copy, PartialEq)]
pub enum FeedShape {
    Grid,
    Bento,
    Masonry,
}

/// Sidebar listing categories as links to their feeds.
#[component]
pub fn CategoryList() -> Element {
    let cats = use_resource(list_categories);
    rsx! {
        div { class: "text-sm",
            h3 { class: "mb-2 font-semibold text-white/80", "Categories" }
            {sidebar_states!(cats, empty: "None yet", list => rsx! {
                ul { class: "space-y-1",
                    for c in list {
                        li { key: "{c.id}",
                            Link {
                                to: Route::CategoryFeed { slug: c.slug.clone() },
                                class: "text-white/60 hover:text-white hover:underline",
                                "{c.name}"
                            }
                        }
                    }
                }
            })}
        }
    }
}

/// Sidebar listing tags as a wrap of pill chips that link to their feeds.
#[component]
pub fn TagList() -> Element {
    let tags = use_resource(list_tags);
    // Highlight the tag whose feed is currently open, in the site accent.
    let active_slug = match use_route::<Route>() {
        Route::TagFeed { slug } => Some(slug),
        _ => None,
    };
    rsx! {
        div { class: "mt-6 text-sm",
            h3 { class: "mb-3 font-semibold text-white/80", "Tags" }
            {sidebar_states!(tags, empty: "None yet", list => rsx! {
                div { class: "flex flex-wrap gap-2",
                    for t in list {
                        TagPill {
                            key: "{t.slug}",
                            name: t.name.clone(),
                            slug: t.slug.clone(),
                            active: active_slug.as_deref() == Some(t.slug.as_str()),
                        }
                    }
                }
            })}
        }
    }
}

/// A single rounded tag chip. The active tag (the feed currently being viewed)
/// is filled with the site accent — the `brand-*` palette driven by the
/// user-set `--brand-hue`; the rest are subtle and pick up an accent tint on
/// hover.
#[component]
fn TagPill(name: String, slug: String, active: bool) -> Element {
    let class = if active {
        "rounded-full border border-brand-500 bg-brand-500 px-3.5 py-1.5 font-medium text-white shadow-sm shadow-brand-500/30"
    } else {
        "rounded-full border border-white/10 bg-white/[0.06] px-3.5 py-1.5 text-white/70 transition-colors hover:border-brand-400/40 hover:bg-white/10 hover:text-white"
    };
    rsx! {
        Link { to: Route::TagFeed { slug }, class, "{name}" }
    }
}

/// Sidebar list of the most-viewed published posts.
#[component]
pub fn FeaturedPosts() -> Element {
    let featured = use_resource(|| async move { featured_posts(5).await });
    rsx! {
        div { class: "text-sm",
            h3 { class: "mb-2 font-semibold text-white/80", "Featured" }
            {sidebar_states!(featured, empty: "No posts yet", list => rsx! {
                ul { class: "space-y-2",
                    for p in list {
                        li { key: "{p.id}",
                            Link {
                                to: Route::PostDetail { slug: p.slug.clone() },
                                class: "text-white/60 hover:text-white hover:underline",
                                "{p.title}"
                            }
                        }
                    }
                }
            })}
        }
    }
}

/// Sidebar list of the most recent approved comments across the blog.
#[component]
pub fn RecentComments() -> Element {
    let recent = use_resource(|| async move { recent_comments(5).await });
    rsx! {
        div { class: "mt-6 text-sm",
            h3 { class: "mb-2 font-semibold text-white/80", "Recent comments" }
            {sidebar_states!(recent, empty: "No comments yet", list => rsx! {
                ul { class: "space-y-3",
                    for c in list {
                        li { key: "{c.id}",
                            p { class: "line-clamp-2 text-white/60", "“{c.body}”" }
                            div { class: "mt-0.5 text-xs text-white/40",
                                span { "{c.display_name} on " }
                                Link {
                                    to: Route::PostDetail { slug: c.post_slug.clone() },
                                    class: "hover:underline",
                                    "{c.post_title}"
                                }
                            }
                        }
                    }
                }
            })}
        }
    }
}

/// A featured/inline image with modern-format responsive sources.
///
/// When `srcset_webp` / `srcset_avif` are present (a local upload with generated
/// renditions), it renders a `<picture>` that lets the browser pick the best
/// format and the smallest file that fills the slot; otherwise it falls back to a
/// plain lazy `<img>` (external URLs, or uploads not yet processed). The
/// `<picture>` is emitted as trusted HTML — every value is escaped and the
/// `srcset`s come from our own `/uploads/…` rendition records.
#[component]
pub fn ResponsiveImg(
    src: String,
    alt: String,
    class: String,
    #[props(default)] sizes: Option<String>,
    #[props(default)] srcset_webp: Option<String>,
    #[props(default)] srcset_avif: Option<String>,
) -> Element {
    if srcset_webp.is_none() && srcset_avif.is_none() {
        return rsx! {
            img {
                class: "{class}",
                src: "{src}",
                alt: "{alt}",
                "loading": "lazy",
                "decoding": "async",
            }
        };
    }

    let sizes = sizes.unwrap_or_else(|| "100vw".to_string());
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('"', "&quot;")
    };
    let mut html = String::from("<picture>");
    if let Some(a) = &srcset_avif {
        html.push_str(&format!(
            r#"<source type="image/avif" srcset="{}" sizes="{}">"#,
            esc(a),
            esc(&sizes)
        ));
    }
    if let Some(w) = &srcset_webp {
        html.push_str(&format!(
            r#"<source type="image/webp" srcset="{}" sizes="{}">"#,
            esc(w),
            esc(&sizes)
        ));
    }
    html.push_str(&format!(
        r#"<img class="{}" src="{}" alt="{}" loading="lazy" decoding="async"></picture>"#,
        esc(&class),
        esc(&src),
        esc(&alt)
    ));
    // `display: contents` so the wrapper doesn't disturb the surrounding flex/grid
    // layout — the `<picture>` behaves as a direct child.
    rsx! { div { class: "contents", dangerous_inner_html: "{html}" } }
}

/// A single post summary card linking to the detail page.
#[component]
pub fn PostCardView(card: PostCard, #[props(default)] fill: bool) -> Element {
    let PostCard {
        title,
        slug,
        excerpt,
        featured_image_url,
        featured_srcset_webp,
        featured_srcset_avif,
        author_name,
        author_username,
        category_name,
        published_at,
        ..
    } = card;

    rsx! {
        article {
            class: "overflow-hidden rounded-xl border border-white/10 bg-white/[0.03]",
            class: if fill { "flex h-full flex-col" },
            if let Some(img) = featured_image_url {
                ResponsiveImg {
                    src: img,
                    alt: title.clone(),
                    class: "h-40 w-full object-cover".to_string(),
                    sizes: "(max-width: 768px) 100vw, 320px".to_string(),
                    srcset_webp: featured_srcset_webp,
                    srcset_avif: featured_srcset_avif,
                }
            }
            div { class: "p-4",
                if let Some(cat) = category_name {
                    span { class: "text-xs uppercase tracking-wide text-brand-400", "{cat}" }
                }
                h2 { class: "mt-1 text-lg font-semibold",
                    Link { to: Route::PostDetail { slug: slug.clone() }, class: "hover:underline", "{title}" }
                }
                p { class: "mt-2 line-clamp-3 text-sm text-white/60", "{excerpt}" }
                div { class: "mt-3 flex items-center gap-2 text-xs text-white/40",
                    Link {
                        to: Route::AuthorProfile { slug: author_username.clone() },
                        class: "hover:underline",
                        "{author_name}"
                    }
                    if let Some(when) = published_at.as_deref().map(crate::model::fmt_date) {
                        span { "·" }
                        span { "{when}" }
                    }
                }
            }
        }
    }
}

/// Prev/next pager driving a page signal in the parent.
#[component]
pub fn PaginationBar(page: i64, total_pages: i64, on_change: EventHandler<i64>) -> Element {
    if total_pages <= 1 {
        return rsx! {};
    }
    rsx! {
        nav { class: "mt-8 flex items-center justify-center gap-4",
            button {
                class: "rounded border border-white/15 px-3 py-1 text-sm disabled:opacity-40",
                disabled: page <= 1,
                onclick: move |_| on_change.call(page - 1),
                "← Prev"
            }
            span { class: "text-sm text-white/60", "Page {page} of {total_pages}" }
            button {
                class: "rounded border border-white/15 px-3 py-1 text-sm disabled:opacity-40",
                disabled: page >= total_pages,
                onclick: move |_| on_change.call(page + 1),
                "Next →"
            }
        }
    }
}

/// Pulsing placeholder shaped like a `PostCardView`, shown while a feed loads.
#[component]
pub fn PostCardSkeleton() -> Element {
    rsx! {
        article { class: "overflow-hidden rounded-xl border border-white/10 bg-white/[0.03]",
            Skeleton { style: "height: 10rem; width: 100%; border-radius: 0;" }
            div { class: "space-y-3 p-4",
                Skeleton { style: "height: 0.75rem; width: 4rem;" }
                Skeleton { style: "height: 1.25rem; width: 80%;" }
                Skeleton { style: "height: 0.75rem; width: 100%;" }
                Skeleton { style: "height: 0.75rem; width: 90%;" }
            }
        }
    }
}

/// A grid of `count` card skeletons matching `FeedGrid`'s layout.
#[component]
pub fn FeedSkeleton(#[props(default = 4)] count: usize) -> Element {
    rsx! {
        div { class: "grid gap-6 sm:grid-cols-2",
            for i in 0..count {
                PostCardSkeleton { key: "{i}" }
            }
        }
    }
}

/// Renders the empty and populated states for a post feed body (loading and
/// error states are handled upstream by the `feed_states!` macro).
#[component]
pub fn FeedGrid(cards: Vec<PostCard>) -> Element {
    if cards.is_empty() {
        return rsx! { p { class: "text-white/50", "No posts yet." } };
    }
    rsx! {
        div { class: "grid gap-6 sm:grid-cols-2",
            for card in cards {
                PostCardView { key: "{card.id}", card }
            }
        }
    }
}

/// A complete paginated feed: the load / error / skeleton states (via
/// `feed_states!`) wrapped around a [`FeedBody`], with the pager wired back to
/// the caller's `page` signal. Every reader feed and the home feed render through
/// this, so the `feed_states!(posts, feed => … FeedBody { … on_change: page.set })`
/// block lives in exactly one place. The caller still owns `page` — its fetch
/// closure reads `page()` to load the right slice — and the `posts` resource;
/// this just renders them and drives `page` on pager clicks.
#[component]
pub fn FeedSection(
    posts: Resource<Result<PostFeed>>,
    shape: FeedShape,
    mut page: Signal<i64>,
) -> Element {
    feed_states!(posts, feed => {
        let cards = feed.items.clone();
        let total_pages = feed.total_pages();
        rsx! {
            FeedBody { shape, cards, page: page(), total_pages, on_change: move |p| page.set(p) }
        }
    })
}

/// A page of feed cards plus its pager, arranged per [`FeedShape`]. This is the
/// one place the Grid / Bento / Masonry card+pager markup lives — the home page
/// and every reader feed render through it instead of repeating the layout-
/// specific `grid-column: 1 / -1` / `column-span: all` pager wrappers.
#[component]
pub fn FeedBody(
    shape: FeedShape,
    cards: Vec<PostCard>,
    page: i64,
    total_pages: i64,
    on_change: EventHandler<i64>,
) -> Element {
    match shape {
        FeedShape::Grid => rsx! {
            FeedGrid { cards }
            PaginationBar { page, total_pages, on_change }
        },
        // Tiles + a full-row pager, placed directly inside `BentoGridLayout`'s grid.
        FeedShape::Bento => rsx! {
            for (i , card) in cards.into_iter().enumerate() {
                div {
                    key: "{card.id}",
                    class: if i == 0 { "col-span-2 row-span-2" } else { "" },
                    PostCardView { card, fill: true }
                }
            }
            div { style: "grid-column: 1 / -1;",
                PaginationBar { page, total_pages, on_change }
            }
        },
        // Column items + a `column-span: all` pager, placed inside `MasonryLayout`.
        FeedShape::Masonry => rsx! {
            for card in cards {
                div {
                    key: "{card.id}",
                    class: "mb-4 inline-block w-full break-inside-avoid",
                    PostCardView { card }
                }
            }
            div { style: "column-span: all;",
                PaginationBar { page, total_pages, on_change }
            }
        },
    }
}
