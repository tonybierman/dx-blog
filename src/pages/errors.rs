//! Error pages (FullBleed): catch-all 404 and an explicit /500.

use dioxus::prelude::*;

use crate::layouts::FullBleedLayout;
use crate::Route;

#[component]
pub fn NotFound(segments: Vec<String>) -> Element {
    let path = segments.join("/");
    rsx! {
        FullBleedLayout {
            div { class: "flex min-h-screen flex-col items-center justify-center gap-3 p-4 text-center",
                h1 { class: "text-5xl font-bold", "404" }
                p { class: "text-white/60", "No page at /{path}" }
                Link { to: Route::HomePage, class: "underline", "Go home" }
            }
        }
    }
}

#[component]
pub fn ServerError(
    /// Optional error detail. Empty for the bare `/500` route; populated when
    /// rendered as the app-wide `ErrorBoundary` fallback.
    #[props(default = String::new())]
    detail: String,
) -> Element {
    rsx! {
        FullBleedLayout {
            div { class: "flex min-h-screen flex-col items-center justify-center gap-3 p-4 text-center",
                h1 { class: "text-5xl font-bold", "500" }
                p { class: "text-white/60", "Something went wrong on our end." }
                if !detail.is_empty() {
                    p { class: "max-w-xl text-sm text-white/40", "{detail}" }
                }
                Link { to: Route::HomePage, class: "underline", "Go home" }
            }
        }
    }
}
