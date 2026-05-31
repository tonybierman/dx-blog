//! Home feed. The post feed itself (`HomeFeed`) is fixed; the structural shell it
//! renders inside is chosen by an admin in Settings and stored in `site_settings`
//! (see [`crate::model::HomeLayout`] / `get_home_layout`). `HomePage` reads that
//! setting and dispatches to the matching layout wrapper from [`crate::layouts`].

use dioxus::prelude::*;

use crate::components::text::PageTitle;
use crate::layouts::{
    BentoGridLayout, CardGridLayout, DrawerLayout, EditorialLayout, FullBleedLayout,
    HeroScrollLayout, HolyGrailLayout, MasonryLayout, MegaMenuLayout, ScrollStickyLayout,
    SplitScreenLayout, StickySidebarLayout,
};
use crate::model::HomeLayout;
use crate::pages::widgets::{
    CategoryList, FeaturedPosts, FeedSection, FeedShape, RecentComments, TagList,
};
use crate::server::posts::list_posts;
use crate::server::settings::get_home_layout;

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
    let content = match layout {
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
                PageTitle { "Latest posts" }
                {body}
            }
        },
        HomeLayout::StickySidebar => rsx! {
            StickySidebarLayout {
                nav: rsx! {
                    CategoryList {}
                    TagList {}
                },
                PageTitle { "Latest posts" }
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
                PageTitle { "Latest posts" }
                {body}
            }
        },
        HomeLayout::FullBleed => rsx! {
            FullBleedLayout {
                div { class: "mx-auto w-full max-w-5xl px-4 py-10",
                    PageTitle { "Latest posts" }
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
                PageTitle { "Latest posts" }
                {body}
            }
        },
        HomeLayout::MegaMenu => rsx! {
            MegaMenuLayout {
                panel: rsx! {
                    CategoryList {}
                    TagList {}
                },
                PageTitle { "Latest posts" }
                {body}
            }
        },
        HomeLayout::BentoGrid => rsx! {
            BentoGridLayout {
                left: rsx! { PageTitle { "Latest posts" } },
                {body}
            }
        },
        HomeLayout::Masonry => rsx! {
            MasonryLayout {
                PageTitle { "Latest posts" }
                {body}
            }
        },
        HomeLayout::CardGrid => rsx! {
            CardGridLayout {
                PageTitle { "Latest posts" }
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
                PageTitle { "Latest posts" }
                {body}
            }
        },
        HomeLayout::ScrollSticky => rsx! {
            ScrollStickyLayout {
                visual: rsx! {
                    FeaturedPosts {}
                    RecentComments {}
                },
                PageTitle { "Latest posts" }
                {body}
            }
        },
    };

    rsx! {
        // Home/site-level Open Graph tags, kept in their own suspense boundary so
        // the feed renders immediately while the (server-resolved) head tags load.
        SuspenseBoundary { fallback: |_| rsx! {}, HomeMeta {} }
        {content}
    }
}

/// The home page's `<head>`: title and the site-level Open Graph / description
/// tags. Server-resolved (`use_server_future`) so they appear in the SSR HTML.
#[component]
fn HomeMeta() -> Element {
    let meta = use_server_future(crate::server::settings::get_site_meta)?;
    let (title, description, url) = match &*meta.read() {
        Some(Ok(m)) => {
            let description = if m.tagline.is_empty() {
                format!("{} — the latest writing from the blog.", m.title)
            } else {
                m.tagline.clone()
            };
            (m.title.clone(), description, format!("{}/", m.base_url))
        }
        _ => (
            crate::server::settings::DEFAULT_SITE_TITLE.to_string(),
            String::new(),
            String::new(),
        ),
    };

    rsx! {
        document::Title { "{title}" }
        document::Meta { property: "og:type", content: "website" }
        document::Meta { property: "og:title", content: "{title}" }
        // Only emit description / og:url when we actually have them — on the
        // `get_site_meta` error fallback they're empty, and a blank meta tag is
        // worse than none (it advertises an empty description / a bad URL).
        if !description.is_empty() {
            document::Meta { name: "description", content: "{description}" }
            document::Meta { property: "og:description", content: "{description}" }
        }
        if !url.is_empty() {
            document::Meta { property: "og:url", content: "{url}" }
        }
    }
}

/// Loads the published-post feed and renders it in the requested `shape`, with a
/// pager below. Shared by every home layout so the load/pagination logic lives in
/// one place.
#[component]
fn HomeFeed(shape: FeedShape) -> Element {
    let page = use_signal(|| 1i64);
    let posts = use_resource(move || async move { list_posts(page(), None, None).await });

    rsx! { FeedSection { posts, shape, page } }
}
