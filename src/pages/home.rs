//! Home feed (HolyGrail): paginated published posts, with category/tag sidebar.

use dioxus::prelude::*;

use crate::layouts::HolyGrailLayout;
use crate::pages::widgets::{CategoryList, FeedGrid, PaginationBar, TagList};
use crate::server::posts::list_posts;

#[component]
pub fn HomePage() -> Element {
    let mut page = use_signal(|| 1i64);
    let posts = use_resource(move || async move { list_posts(page(), None, None).await });

    rsx! {
        HolyGrailLayout {
            left: rsx! {
                CategoryList {}
                TagList {}
            },
            right: rsx! {
                div { class: "text-sm",
                    h3 { class: "mb-2 font-semibold text-white/80", "About" }
                    p { class: "text-white/50", "A blog built on Dioxus Fullstack." }
                }
            },
            h1 { class: "mb-6 text-2xl font-bold", "Latest posts" }
            match &*posts.read() {
                Some(Ok(feed)) => {
                    let cards = feed.items.clone();
                    let total_pages = feed.total_pages();
                    rsx! {
                        FeedGrid { cards }
                        PaginationBar {
                            page: page(),
                            total_pages,
                            on_change: move |p| page.set(p),
                        }
                    }
                }
                Some(Err(e)) => rsx! { p { class: "text-red-400", "Failed to load posts: {e}" } },
                None => rsx! { p { class: "text-white/50", "Loading…" } },
            }
        }
    }
}
