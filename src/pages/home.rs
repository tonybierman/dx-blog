//! Home feed. The post feed itself (`HomeFeed`) is fixed; the structural shell it
//! renders inside is chosen by an admin in Settings and stored in `site_settings`
//! (see [`crate::model::HomeLayout`] / `get_home_layout`). `HomePage` reads that
//! setting and dispatches to the matching layout wrapper from [`crate::layouts`].

use dioxus::prelude::*;

use crate::layouts::{
    BentoGridLayout, CardGridLayout, DrawerLayout, EditorialLayout, FullBleedLayout,
    HeroScrollLayout, HolyGrailLayout, MasonryLayout, MegaMenuLayout, ScrollStickyLayout,
    SplitScreenLayout, StickySidebarLayout,
};
use crate::model::HomeLayout;
use crate::pages::widgets::{
    CategoryList, FeaturedPosts, FeedGrid, FeedSkeleton, PaginationBar, PostCardView,
    RecentComments, TagList,
};
use crate::server::posts::list_posts;
use crate::server::settings::get_home_layout;

/// How the feed cards are arranged inside the chosen layout. Most layouts render
/// the feed as a responsive `FeedGrid`; the Bento and Masonry layouts arrange the
/// cards themselves (and need a full-width pager), mirroring `reader.rs`.
#[derive(Clone, Copy, PartialEq)]
enum FeedShape {
    Grid,
    Bento,
    Masonry,
}

#[component]
pub fn HomePage() -> Element {
    let layout_res = use_resource(get_home_layout);
    // Default to Holy Grail while the setting loads (or if it errors) so the page
    // never blanks.
    let layout = match &*layout_res.read() {
        Some(Ok(l)) => *l,
        _ => HomeLayout::default(),
    };

    let shape = match layout {
        HomeLayout::BentoGrid => FeedShape::Bento,
        HomeLayout::Masonry => FeedShape::Masonry,
        _ => FeedShape::Grid,
    };
    let body = rsx! { HomeFeed { shape } };

    // Each arm wraps the same feed `body` in a different structural shell. Only
    // one arm runs, so moving `body` into several arms is fine.
    match layout {
        HomeLayout::HolyGrail => rsx! {
            HolyGrailLayout {
                left: rsx! {
                    CategoryList {}
                    TagList {}
                },
                right: rsx! {
                    FeaturedPosts {}
                    RecentComments {}
                },
                h1 { class: "mb-6 text-2xl font-bold", "Latest posts" }
                {body}
            }
        },
        HomeLayout::StickySidebar => rsx! {
            StickySidebarLayout {
                nav: rsx! {
                    CategoryList {}
                    TagList {}
                },
                h1 { class: "mb-6 text-2xl font-bold", "Latest posts" }
                {body}
            }
        },
        HomeLayout::SplitScreen => rsx! {
            SplitScreenLayout {
                intro: rsx! {
                    div {
                        h1 { class: "text-3xl font-bold", "dx-blog" }
                        p { class: "mt-3 text-white/60", "Latest writing from the blog." }
                    }
                },
                h1 { class: "mb-6 text-2xl font-bold", "Latest posts" }
                {body}
            }
        },
        HomeLayout::FullBleed => rsx! {
            FullBleedLayout {
                div { class: "mx-auto w-full max-w-5xl px-4 py-10",
                    h1 { class: "mb-6 text-2xl font-bold", "Latest posts" }
                    {body}
                }
            }
        },
        HomeLayout::Drawer => rsx! {
            DrawerLayout {
                nav: rsx! {
                    CategoryList {}
                    TagList {}
                },
                h1 { class: "mb-6 text-2xl font-bold", "Latest posts" }
                {body}
            }
        },
        HomeLayout::MegaMenu => rsx! {
            MegaMenuLayout {
                panel: rsx! {
                    CategoryList {}
                    TagList {}
                },
                h1 { class: "mb-6 text-2xl font-bold", "Latest posts" }
                {body}
            }
        },
        HomeLayout::BentoGrid => rsx! {
            BentoGridLayout {
                left: rsx! { h1 { class: "text-2xl font-bold", "Latest posts" } },
                {body}
            }
        },
        HomeLayout::Masonry => rsx! {
            MasonryLayout {
                h1 { class: "mb-6 text-2xl font-bold", "Latest posts" }
                {body}
            }
        },
        HomeLayout::CardGrid => rsx! {
            CardGridLayout {
                h1 { class: "mb-6 text-2xl font-bold", "Latest posts" }
                {body}
            }
        },
        HomeLayout::Editorial => rsx! {
            EditorialLayout {
                sidebar: rsx! {
                    FeaturedPosts {}
                    RecentComments {}
                },
                h1 { class: "mb-6 text-3xl font-bold tracking-tight", "Latest posts" }
                {body}
            }
        },
        HomeLayout::HeroScroll => rsx! {
            HeroScrollLayout {
                h1 { class: "mb-6 text-2xl font-bold", "Latest posts" }
                {body}
            }
        },
        HomeLayout::ScrollSticky => rsx! {
            ScrollStickyLayout {
                visual: rsx! {
                    FeaturedPosts {}
                    RecentComments {}
                },
                h1 { class: "mb-6 text-2xl font-bold", "Latest posts" }
                {body}
            }
        },
    }
}

/// Loads the published-post feed and renders it in the requested `shape`, with a
/// pager below. Shared by every home layout so the load/pagination logic lives in
/// one place.
#[component]
fn HomeFeed(shape: FeedShape) -> Element {
    let mut page = use_signal(|| 1i64);
    let posts = use_resource(move || async move { list_posts(page(), None, None).await });

    // Bind the result so the `read()` guard drops at the statement boundary
    // rather than being held across the function's return.
    let rendered = match &*posts.read() {
        Some(Ok(feed)) => {
            let cards = feed.items.clone();
            let total_pages = feed.total_pages();
            match shape {
                FeedShape::Grid => rsx! {
                    FeedGrid { cards }
                    PaginationBar { page: page(), total_pages, on_change: move |p| page.set(p) }
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
                        PaginationBar { page: page(), total_pages, on_change: move |p| page.set(p) }
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
                        PaginationBar { page: page(), total_pages, on_change: move |p| page.set(p) }
                    }
                },
            }
        }
        Some(Err(e)) => rsx! {
            p { class: "text-red-400", "Failed to load posts: {e}" }
        },
        None => rsx! {
            FeedSkeleton {}
        },
    };
    rendered
}
