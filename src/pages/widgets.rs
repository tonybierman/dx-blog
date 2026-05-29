//! Small shared UI pieces used across the reader pages.

use dioxus::prelude::*;

use arium_dioxus::ui::components::skeleton::Skeleton;

use crate::model::PostCard;
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
pub(crate) use feed_states;

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

/// A single post summary card linking to the detail page.
#[component]
pub fn PostCardView(card: PostCard, #[props(default)] fill: bool) -> Element {
    let PostCard {
        title,
        slug,
        excerpt,
        featured_image_url,
        author_name,
        category_name,
        published_at,
        ..
    } = card;

    rsx! {
        article {
            class: "overflow-hidden rounded-xl border border-white/10 bg-white/[0.03]",
            class: if fill { "flex h-full flex-col" },
            if let Some(img) = featured_image_url {
                img { class: "h-40 w-full object-cover", src: "{img}", alt: "{title}" }
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
                    span { "{author_name}" }
                    if let Some(when) = published_at {
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

/// Renders the loading / error / empty / list states for a post feed body.
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
