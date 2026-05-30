//! Client side of the live-reading experience: one [`use_live`] hook per post
//! that opens a single browser `EventSource` to `/api/live/{post_id}` and pumps
//! the incoming events into signals.
//!
//! Mirrors the SSR/hydration discipline of `crate::embeds`: the signals exist on
//! both build targets so the shared `rsx!` in `crate::pages::reader` compiles
//! everywhere, but the actual connection is `#[cfg(feature = "web")]`. During SSR
//! the hook returns inert defaults (0 readers, no live comments, no claps), so
//! the server-rendered HTML is static and matches the first client paint; the
//! stream only comes alive once the wasm client hydrates. gloo-net's
//! `EventSource` reconnects automatically, and because presence is broadcast as
//! an absolute count, a reconnect resyncs the badge on the next event.

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::model::CommentView;

/// Per-topic cap on buffered live data points. A streaming feed is unbounded, so
/// keep only the most recent points — generously more than any chart's `window`
/// so each embed can slice its own view without the buffer growing forever.
#[cfg(feature = "web")]
const MAX_DATA_POINTS: usize = 256;

/// One floating clap to animate in. Purely a client render artifact (it never
/// crosses the wire), so it lives here rather than in `crate::model`. `id` is a
/// monotonic nonce used as the render key and to prune finished animations.
#[derive(Clone, PartialEq)]
pub struct ClapBurst {
    pub id: u64,
    pub kind: String,
}

/// Live state for a post, handed down to the reader UI. `Signal` is `Copy`, so
/// the whole handle is `Copy` and threads into child components as a prop without
/// cloning or prop-drilling lints. `PartialEq` is required for `#[component]` props.
#[derive(Clone, Copy, PartialEq)]
pub struct LiveHandle {
    /// Distinct readers currently connected to this post.
    pub reading_now: Signal<i64>,
    /// Approved comments pushed since the page loaded (deduped by id).
    pub live_comments: Signal<Vec<CommentView>>,
    /// Claps awaiting / playing their float animation.
    pub claps: Signal<Vec<ClapBurst>>,
    /// Authoritative reaction total, updated from each reaction event. 0 until
    /// the first event arrives; the reader falls back to its initial fetch.
    pub reaction_count: Signal<i64>,
    /// Buffered live data series keyed by topic, newest last. Fed by
    /// `LiveEvent::Data`; `livechart` embeds read their own topic and render a
    /// sliding window. Empty during SSR and until the first point arrives.
    pub data_points: Signal<HashMap<String, Vec<f64>>>,
}

/// Open (on the wasm client) the live channel for `post_id` and expose its state
/// as signals. On the server build this just returns the inert defaults.
pub fn use_live(post_id: i64) -> LiveHandle {
    // Declared unconditionally so `LiveHandle` is identical on both targets.
    let reading_now = use_signal(|| 0i64);
    let live_comments = use_signal(Vec::<CommentView>::new);
    let claps = use_signal(Vec::<ClapBurst>::new);
    let reaction_count = use_signal(|| 0i64);
    let data_points = use_signal(HashMap::<String, Vec<f64>>::new);
    let handle = LiveHandle {
        reading_now,
        live_comments,
        claps,
        reaction_count,
        data_points,
    };

    #[cfg(feature = "web")]
    {
        use futures_util::StreamExt;
        use gloo_net::eventsource::futures::{EventSource, EventSourceSubscription};

        let mut reading_now = reading_now;
        let mut live_comments = live_comments;
        let mut claps = claps;
        let mut reaction_count = reaction_count;
        let mut data_points = data_points;

        use_future(move || async move {
            let Ok(mut es) = EventSource::new(&format!("/api/live/{post_id}")) else {
                return;
            };
            // One stream per named event the server emits; merge them so a single
            // task drains all three. `subscribe` only fails on a duplicate event
            // name, which can't happen here. The subscriptions are `!Unpin`, so
            // `Box::pin` them (no `alloc` combinator feature needed) before
            // `select_all`, which requires `Unpin` streams.
            let streams: Vec<std::pin::Pin<Box<EventSourceSubscription>>> =
                ["presence", "comment", "reaction", "data"]
                    .into_iter()
                    .filter_map(|name| es.subscribe(name).ok())
                    .map(Box::pin)
                    .collect();
            let mut merged = futures_util::stream::select_all(streams);

            // Keep `es` alive for the whole loop — dropping it closes the
            // connection. Held by the async block; only drops when the task ends
            // (component unmount / navigation).
            let _es = &es;

            let mut next_clap_id = 0u64;
            while let Some(Ok((_event_name, msg))) = merged.next().await {
                let Some(text) = msg.data().as_string() else {
                    continue;
                };
                let Ok(event) = serde_json::from_str::<crate::model::LiveEvent>(&text) else {
                    continue;
                };
                match event {
                    crate::model::LiveEvent::Presence { count } => reading_now.set(count),
                    crate::model::LiveEvent::Comment(c) => {
                        // Dedupe the echo of our own just-approved comment, and any
                        // accidental resend after a reconnect.
                        live_comments.with_mut(|v| {
                            if !v.iter().any(|x| x.id == c.id) {
                                v.push(c);
                            }
                        });
                    }
                    crate::model::LiveEvent::Reaction { kind, total } => {
                        reaction_count.set(total);
                        next_clap_id += 1;
                        claps.with_mut(|v| {
                            v.push(ClapBurst {
                                id: next_clap_id,
                                kind,
                            });
                            // Hard cap so a clap storm can't grow the vec without
                            // bound; the reader also time-prunes finished bursts.
                            let overflow = v.len().saturating_sub(40);
                            if overflow > 0 {
                                v.drain(0..overflow);
                            }
                        });
                    }
                    crate::model::LiveEvent::Data { topic, value } => {
                        // Append to this topic's ring of recent samples; charts
                        // reading the topic re-render on the signal write.
                        data_points.with_mut(|m| {
                            let series = m.entry(topic).or_default();
                            series.push(value);
                            let overflow = series.len().saturating_sub(MAX_DATA_POINTS);
                            if overflow > 0 {
                                series.drain(0..overflow);
                            }
                        });
                    }
                }
            }
        });
    }
    // `post_id` is only read on the web target; keep it "used" on the server.
    let _ = post_id;

    handle
}

/// Cap on retained admin activity rows. The feed is "since page load"; older
/// rows drop off the front.
#[cfg(feature = "web")]
const MAX_ACTIVITY: usize = 50;

/// One row in the admin activity feed. A client render artifact (never crosses
/// the wire), like [`ClapBurst`]. `key` is a monotonic render key.
#[derive(Clone, PartialEq)]
pub struct ActivityItem {
    pub key: u64,
    pub post_title: String,
    pub post_slug: String,
    pub kind: ActivityKind,
}

/// What an [`ActivityItem`] represents.
// Constructed only in the web-gated stream loop; on the server build the
// variants are matched (in the dashboard feed) but never built, which reads as
// dead code there.
#[cfg_attr(not(feature = "web"), allow(dead_code))]
#[derive(Clone, PartialEq)]
pub enum ActivityKind {
    /// A comment, with its id (for dedupe/removal) and current moderation status.
    Comment {
        id: i64,
        who: String,
        status: String,
    },
    /// Reactions on a post, coalesced into one row carrying the latest total.
    Reaction { total: i64 },
}

/// Site-wide admin live state for the dashboard / moderation queue. Like
/// [`LiveHandle`] it's all `Signal`s, so the handle is `Copy` and threads into
/// child components as a prop.
#[derive(Clone, Copy, PartialEq)]
pub struct AdminLiveHandle {
    /// Recent activity rows (newest last), capped and coalesced.
    pub activity: Signal<Vec<ActivityItem>>,
    /// Bumped on every comment create/moderate/remove event — pages watch this
    /// to refetch authoritative data (counts, the moderation list).
    pub comment_tick: Signal<u64>,
    /// Count of reaction events seen since load, added to the fetched baseline
    /// for a live site-wide reaction tile (best-effort; drift self-heals on the
    /// next authoritative refetch).
    pub reaction_delta: Signal<i64>,
}

/// Open (on the wasm client) the site-wide admin event channel and expose its
/// state as signals. Connects only when `enabled` (the caller passes whether the
/// user holds `COMMENTS_MODERATE` — the stream 403s otherwise). On the server
/// build, or when not enabled, this returns inert defaults.
pub fn use_admin_live(enabled: bool) -> AdminLiveHandle {
    // Declared unconditionally so the handle type matches on both targets.
    let activity = use_signal(Vec::<ActivityItem>::new);
    let comment_tick = use_signal(|| 0u64);
    let reaction_delta = use_signal(|| 0i64);
    let handle = AdminLiveHandle {
        activity,
        comment_tick,
        reaction_delta,
    };

    #[cfg(feature = "web")]
    {
        use futures_util::StreamExt;
        use gloo_net::eventsource::futures::{EventSource, EventSourceSubscription};

        let mut activity = activity;
        let mut comment_tick = comment_tick;
        let mut reaction_delta = reaction_delta;

        // Reactive on `enabled` so the connection opens once permissions resolve
        // to true (and tears down if it flips back).
        use_future(use_reactive!(|(enabled,)| async move {
            if !enabled {
                return;
            }
            let Ok(mut es) = EventSource::new("/api/admin/live") else {
                return;
            };
            let streams: Vec<std::pin::Pin<Box<EventSourceSubscription>>> =
                ["comment", "reaction", "comment_removed"]
                    .into_iter()
                    .filter_map(|name| es.subscribe(name).ok())
                    .map(Box::pin)
                    .collect();
            let mut merged = futures_util::stream::select_all(streams);
            let _es = &es;

            let mut next_key = 0u64;
            while let Some(Ok((_name, msg))) = merged.next().await {
                let Some(text) = msg.data().as_string() else {
                    continue;
                };
                let Ok(event) = serde_json::from_str::<crate::model::AdminEvent>(&text) else {
                    continue;
                };
                match event {
                    crate::model::AdminEvent::Comment {
                        id,
                        post_title,
                        post_slug,
                        display_name,
                        status,
                        ..
                    } => {
                        comment_tick += 1;
                        activity.with_mut(|v| {
                            // Update in place if we've already seen this comment
                            // (e.g. a status change), else add a new row.
                            if let Some(it) = v.iter_mut().find(|it| {
                                matches!(&it.kind, ActivityKind::Comment { id: cid, .. } if *cid == id)
                            }) {
                                it.kind = ActivityKind::Comment {
                                    id,
                                    who: display_name,
                                    status,
                                };
                            } else {
                                next_key += 1;
                                v.push(ActivityItem {
                                    key: next_key,
                                    post_title,
                                    post_slug,
                                    kind: ActivityKind::Comment {
                                        id,
                                        who: display_name,
                                        status,
                                    },
                                });
                                let overflow = v.len().saturating_sub(MAX_ACTIVITY);
                                if overflow > 0 {
                                    v.drain(0..overflow);
                                }
                            }
                        });
                    }
                    crate::model::AdminEvent::Reaction {
                        post_slug,
                        post_title,
                        total,
                        ..
                    } => {
                        reaction_delta += 1;
                        activity.with_mut(|v| {
                            // Coalesce reactions per post so a clap storm doesn't
                            // flood the feed — update that post's running total.
                            if let Some(it) = v.iter_mut().find(|it| {
                                it.post_slug == post_slug
                                    && matches!(it.kind, ActivityKind::Reaction { .. })
                            }) {
                                it.kind = ActivityKind::Reaction { total };
                            } else {
                                next_key += 1;
                                v.push(ActivityItem {
                                    key: next_key,
                                    post_title,
                                    post_slug,
                                    kind: ActivityKind::Reaction { total },
                                });
                                let overflow = v.len().saturating_sub(MAX_ACTIVITY);
                                if overflow > 0 {
                                    v.drain(0..overflow);
                                }
                            }
                        });
                    }
                    crate::model::AdminEvent::CommentRemoved { id } => {
                        comment_tick += 1;
                        activity.with_mut(|v| {
                            v.retain(|it| {
                                !matches!(&it.kind, ActivityKind::Comment { id: cid, .. } if *cid == id)
                            });
                        });
                    }
                }
            }
        }));
    }
    // `enabled` is only read on the web target; keep it "used" on the server.
    let _ = enabled;

    handle
}
