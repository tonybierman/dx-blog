//! Small shared UI pieces used across the reader pages.

use dioxus::prelude::*;

use arium_dioxus::ui::components::skeleton::Skeleton;

use crate::model::PostCard;
use crate::server::comments::recent_comments;
use crate::server::posts::featured_posts;
use crate::server::taxonomy::{list_categories, list_tags};
use crate::Route;

/// Sidebar listing categories as links to their feeds.
#[component]
pub fn CategoryList() -> Element {
    let cats = use_resource(list_categories);
    rsx! {
        div { class: "text-sm",
            h3 { class: "mb-2 font-semibold text-white/80", "Categories" }
            match &*cats.read() {
                Some(Ok(list)) if !list.is_empty() => rsx! {
                    ul { class: "space-y-1",
                        for c in list.clone() {
                            li {
                                Link {
                                    to: Route::CategoryFeed { slug: c.slug.clone() },
                                    class: "text-white/60 hover:text-white hover:underline",
                                    "{c.name}"
                                }
                            }
                        }
                    }
                },
                Some(Ok(_)) => rsx! { p { class: "text-white/40", "None yet" } },
                Some(Err(_)) => rsx! { p { class: "text-white/40", "—" } },
                None => rsx! { p { class: "text-white/40", "…" } },
            }
        }
    }
}

/// Sidebar listing tags as links to their feeds.
#[component]
pub fn TagList() -> Element {
    let tags = use_resource(list_tags);
    rsx! {
        div { class: "mt-6 text-sm",
            h3 { class: "mb-2 font-semibold text-white/80", "Tags" }
            match &*tags.read() {
                Some(Ok(list)) if !list.is_empty() => rsx! {
                    div { class: "flex flex-wrap gap-2",
                        for t in list.clone() {
                            Link {
                                to: Route::TagFeed { slug: t.slug.clone() },
                                class: "rounded-full border border-white/15 px-2 py-0.5 text-xs text-white/60 hover:text-white",
                                "#{t.name}"
                            }
                        }
                    }
                },
                Some(Ok(_)) => rsx! { p { class: "text-white/40", "None yet" } },
                Some(Err(_)) => rsx! { p { class: "text-white/40", "—" } },
                None => rsx! { p { class: "text-white/40", "…" } },
            }
        }
    }
}

/// Sidebar list of the most-viewed published posts.
#[component]
pub fn FeaturedPosts() -> Element {
    let featured = use_resource(|| async move { featured_posts(5).await });
    rsx! {
        div { class: "text-sm",
            h3 { class: "mb-2 font-semibold text-white/80", "Featured" }
            match &*featured.read() {
                Some(Ok(list)) if !list.is_empty() => rsx! {
                    ul { class: "space-y-2",
                        for p in list.clone() {
                            li { key: "{p.id}",
                                Link {
                                    to: Route::PostDetail { slug: p.slug.clone() },
                                    class: "text-white/60 hover:text-white hover:underline",
                                    "{p.title}"
                                }
                            }
                        }
                    }
                },
                Some(Ok(_)) => rsx! { p { class: "text-white/40", "No posts yet" } },
                Some(Err(_)) => rsx! { p { class: "text-white/40", "—" } },
                None => rsx! { p { class: "text-white/40", "…" } },
            }
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
            match &*recent.read() {
                Some(Ok(list)) if !list.is_empty() => rsx! {
                    ul { class: "space-y-3",
                        for c in list.clone() {
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
                },
                Some(Ok(_)) => rsx! { p { class: "text-white/40", "No comments yet" } },
                Some(Err(_)) => rsx! { p { class: "text-white/40", "—" } },
                None => rsx! { p { class: "text-white/40", "…" } },
            }
        }
    }
}

/// A single post summary card linking to the detail page.
#[component]
pub fn PostCardView(card: PostCard) -> Element {
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
        article { class: "overflow-hidden rounded-xl border border-white/10 bg-white/[0.03]",
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
