//! Dashboard (metric tiles) and the fuller analytics view (tiles + views-over-
//! time chart + top posts/referrers).

use dioxus::prelude::*;

use crate::model::AnalyticsSummary;
use crate::server::analytics::{analytics_summary, top_posts, top_referrers, views_over_time};

use super::AdminShell;

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
