//! Dashboard (metric tiles) and the fuller analytics view (tiles + views-over-
//! time chart + top posts/referrers).

use dioxus::prelude::*;

use arium_dioxus::ui::{use_permissions, UsePermissions};

use crate::auth_tokens::{ANALYTICS_READ, COMMENTS_MODERATE};
use crate::live::{use_admin_live, ActivityKind, AdminLiveHandle};
use crate::model::AnalyticsSummary;
use crate::server::analytics::{analytics_summary, top_posts, top_referrers, views_over_time};
use crate::Route;

use super::{admin_landing, AdminShell};

/// These pages are backed by analytics server fns. A user without
/// `ANALYTICS_READ` (their nav links are already hidden) can still reach them by
/// direct URL — e.g. landing on `/admin`. Bounce them to their first accessible
/// section instead of rendering the raw "permission" error. Returns `None`
/// while permissions load, when the user holds the token, or when they have no
/// admin section at all (the shell's route guard then sends them to login).
fn analytics_redirect(perms: &UsePermissions) -> Option<Route> {
    (!perms.is_loading() && !perms.has(ANALYTICS_READ))
        .then(|| admin_landing(|t| perms.has(t)))
        .flatten()
}

#[component]
fn MetricTiles(summary: AnalyticsSummary) -> Element {
    let tiles = [
        ("Posts", summary.post_count),
        ("Published", summary.published_count),
        ("Drafts", summary.draft_count),
        ("Comments", summary.comment_count),
        ("Pending", summary.pending_comment_count),
        ("Reactions", summary.reaction_count),
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
    let perms = use_permissions();
    let mut summary = use_resource(analytics_summary);
    // The live admin stream carries comment bodies' metadata + reactions and is
    // gated on COMMENTS_MODERATE, so only connect (and show the feed) for users
    // who hold it; analytics-only users keep static tiles.
    let has_moderate = perms.has(COMMENTS_MODERATE);
    let live = use_admin_live(has_moderate);

    // Comments are rare → refetch authoritative counts on each comment event.
    // (Reactions are frequent and handled by a local delta below, no refetch.)
    use_effect(move || {
        let _ = (live.comment_tick)();
        summary.restart();
    });

    if let Some(route) = analytics_redirect(&perms) {
        navigator().replace(route);
        return rsx! {};
    }
    rsx! {
        AdminShell { active: "dashboard".to_string(),
            h1 { class: "mb-6 text-2xl font-bold", "Dashboard" }
            match &*summary.read() {
                Some(Ok(s)) => {
                    // Fold the live reaction delta into the fetched baseline so
                    // the Reactions tile ticks up between authoritative refetches.
                    let mut s = s.clone();
                    s.reaction_count += (live.reaction_delta)();
                    rsx! { MetricTiles { summary: s } }
                }
                Some(Err(e)) => rsx! { p { class: "text-red-400", "{e}" } },
                None => rsx! { p { class: "text-white/50", "Loading…" } },
            }
            if has_moderate {
                ActivityFeed { live }
            }
        }
    }
}

/// Real-time feed of incoming comments and reactions (notifications only — no
/// comment bodies). Only rendered for `COMMENTS_MODERATE` holders.
#[component]
fn ActivityFeed(live: AdminLiveHandle) -> Element {
    let items = (live.activity)();
    rsx! {
        section { class: "mt-8",
            h2 { class: "mb-3 text-lg font-semibold", "Live activity" }
            div { class: "rounded-xl border border-white/10 bg-white/[0.03] p-2",
                if items.is_empty() {
                    p { class: "p-3 text-sm text-white/40", "Waiting for activity…" }
                } else {
                    ul { class: "divide-y divide-white/5",
                        // Newest first.
                        for it in items.into_iter().rev() {
                            li { key: "{it.key}", class: "flex items-center justify-between gap-3 px-3 py-2 text-sm",
                                div { class: "flex min-w-0 items-center gap-2",
                                    // All activity badges use the theme accent
                                    // (brand-*), tinted like the presence badge.
                                    match &it.kind {
                                        ActivityKind::Comment { who, status, .. } => rsx! {
                                            span { class: "rounded bg-brand-500/10 px-1.5 text-xs text-brand-300",
                                                "{status}"
                                            }
                                            span { class: "truncate text-white/70", "💬 {who}" }
                                        },
                                        ActivityKind::Reaction { total } => rsx! {
                                            span { class: "rounded bg-brand-500/10 px-1.5 text-xs text-brand-300", "reaction" }
                                            span { class: "truncate text-white/70", "👏 {total} total" }
                                        },
                                    }
                                }
                                Link {
                                    to: Route::PostDetail { slug: it.post_slug.clone() },
                                    class: "shrink-0 truncate text-white/40 hover:underline",
                                    "{it.post_title}"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn AdminAnalytics() -> Element {
    let perms = use_permissions();
    let summary = use_resource(analytics_summary);
    let top = use_resource(top_posts);
    let referrers = use_resource(top_referrers);
    let series = use_resource(views_over_time);
    if let Some(route) = analytics_redirect(&perms) {
        navigator().replace(route);
        return rsx! {};
    }
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
