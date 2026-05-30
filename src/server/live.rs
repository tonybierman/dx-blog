//! The live-reading hub and its SSE endpoint.
//!
//! The whole real-time layer hangs off a single in-memory [`LiveHub`]: one
//! `tokio::sync::broadcast` channel per post plus an ephemeral presence tally.
//! It's installed as an `axum::Extension<Arc<LiveHub>>` (see `main.rs`) so both
//! this raw SSE route and the ordinary Dioxus server fns (`create_comment`,
//! `add_reaction`, `moderate_comment`) can reach it via [`HubExtension`].
//!
//! [`live_handler`] is a plain axum GET, not a server fn — SSE needs a streaming
//! `Response` the `#[get]` macro can't model, exactly like the XML feed handlers
//! in `crate::server::feeds`. A client connects to `/api/live/{post_id}`,
//! [`LiveHub::subscribe`] registers its presence and hands back a receiver; when
//! the connection drops, the [`PresenceGuard`] held by the stream decrements the
//! tally and the new count is broadcast to everyone still reading.
//!
//! Presence is purely in-memory and single-process (correct for this deploy) —
//! it never touches the DB. Reactions/comments persist through their own server
//! fns; the hub only fans out the notification.

#![cfg(feature = "server")]

use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::extract::{Extension, Path};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::{stream, Stream, StreamExt};
use tokio::sync::broadcast;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;

use crate::model::LiveEvent;

/// The `axum::Extension` that carries the shared hub to every route and server
/// fn. Mirrors `DbExtension`/`MailExtension` in `crate::server`.
pub type HubExtension = Extension<Arc<LiveHub>>;

/// Per-post broadcast capacity. Live traffic is tiny (presence flips, the
/// occasional comment, claps), so this is generous headroom; a client that falls
/// this far behind gets a `Lagged` it skips, then resyncs on the next event
/// (presence is absolute, so it self-heals immediately).
const CHANNEL_CAP: usize = 256;

/// Per-(post, topic) cap on retained live-data points. Bounds both the backlog
/// replayed to a new connection and the hub's memory. Matches the client's
/// `MAX_DATA_POINTS` so a fresh reader can be handed a full client buffer.
const HISTORY_MAX: usize = 256;

/// One post's fan-out channel plus its live-reader tally.
struct PostChannel {
    tx: broadcast::Sender<LiveEvent>,
    /// `visitor_hash` -> number of open connections from that visitor. "Reading
    /// now" is the number of *distinct* visitors, i.e. `presence.len()`, so two
    /// tabs from one browser count once.
    presence: HashMap<String, u32>,
}

/// All hub state, behind one mutex so a [`subscribe`](LiveHub::subscribe)
/// snapshot and a [`publish`](LiveHub::publish) can't interleave — that's what
/// makes the backlog/live cut exact (no point dropped or duplicated on connect).
#[derive(Default)]
struct HubState {
    /// Live fan-out channels, created on first subscribe and reclaimed when the
    /// last reader of a post leaves.
    channels: HashMap<i64, PostChannel>,
    /// Recent `Data` points per post/topic, newest last. Unlike `channels` this
    /// is *retained* across presence GC, so a reader who connects when nobody
    /// else is present (or who is the first to arrive after a producer started)
    /// still gets backfilled. Capped per topic by `HISTORY_MAX`; entries persist
    /// for any post a producer has ever pushed to (fine for the handful of posts
    /// that host a live chart — add GC if that ever stops being true).
    history: HashMap<i64, HashMap<String, VecDeque<f64>>>,
}

/// In-memory registry of per-post live channels. Cheap to clone (it's always
/// behind an `Arc`); all state is guarded by one `Mutex`.
#[derive(Default)]
pub struct LiveHub {
    state: Mutex<HubState>,
}

impl LiveHub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Subscribe to a post's events and register one unit of presence. Returns
    /// the receiver to stream from, a guard whose `Drop` removes the presence
    /// again, and the retained data backlog as a list of `Data` events to replay
    /// to this connection before live events flow. The current (post-increment)
    /// reader count is broadcast so the just-joined client — and everyone already
    /// reading — sees it immediately.
    ///
    /// The backlog snapshot and the `tx.subscribe()` happen under the same lock,
    /// so relative to any `publish` the cut is exact: a point is either in the
    /// returned backlog or delivered on `rx`, never both and never neither.
    pub fn subscribe(
        self: &Arc<Self>,
        post_id: i64,
        visitor: String,
    ) -> (
        broadcast::Receiver<LiveEvent>,
        PresenceGuard,
        Vec<LiveEvent>,
    ) {
        let (rx, count, backlog) = {
            let mut state = self.state.lock().unwrap();
            let backlog = state
                .history
                .get(&post_id)
                .map(|topics| {
                    topics
                        .iter()
                        .flat_map(|(topic, pts)| {
                            pts.iter().map(move |&value| LiveEvent::Data {
                                topic: topic.clone(),
                                value,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let ch = state
                .channels
                .entry(post_id)
                .or_insert_with(|| PostChannel {
                    tx: broadcast::channel(CHANNEL_CAP).0,
                    presence: HashMap::new(),
                });
            let rx = ch.tx.subscribe();
            *ch.presence.entry(visitor.clone()).or_insert(0) += 1;
            (rx, ch.presence.len() as i64, backlog)
        }; // lock released before publish() re-locks — never held across the send.
        self.publish(post_id, LiveEvent::Presence { count });
        (
            rx,
            PresenceGuard {
                hub: Arc::clone(self),
                post_id,
                visitor,
            },
            backlog,
        )
    }

    /// Fan an event out to every subscriber of `post_id`. `Data` points are also
    /// appended to the retained per-topic history (so late joiners get them as
    /// backlog) regardless of whether anyone is currently connected. The live
    /// send is a no-op if the post has no channel or no receivers.
    pub fn publish(&self, post_id: i64, event: LiveEvent) {
        let mut state = self.state.lock().unwrap();
        if let LiveEvent::Data { topic, value } = &event {
            let series = state
                .history
                .entry(post_id)
                .or_default()
                .entry(topic.clone())
                .or_default();
            series.push_back(*value);
            while series.len() > HISTORY_MAX {
                series.pop_front();
            }
        }
        if let Some(ch) = state.channels.get(&post_id) {
            let _ = ch.tx.send(event);
        }
    }
}

/// Held for the lifetime of an SSE connection. Its `Drop` decrements the post's
/// presence and broadcasts the new count, and reclaims the channel once the last
/// reader leaves so the hub can't grow without bound.
pub struct PresenceGuard {
    hub: Arc<LiveHub>,
    post_id: i64,
    visitor: String,
}

impl Drop for PresenceGuard {
    fn drop(&mut self) {
        let new_count = {
            let mut state = self.hub.state.lock().unwrap();
            let Some(ch) = state.channels.get_mut(&self.post_id) else {
                return;
            };
            if let Some(n) = ch.presence.get_mut(&self.visitor) {
                *n -= 1;
                if *n == 0 {
                    ch.presence.remove(&self.visitor);
                }
            }
            let count = ch.presence.len() as i64;
            // Reclaim the channel when nobody is left. `receiver_count()` is
            // accurate here because the SSE stream drops its `BroadcastStream`
            // (and thus the receiver) before dropping this guard — see the field
            // order in `LiveStream`. The post's data `history` is intentionally
            // NOT removed here, so the next reader to arrive still gets backfill.
            if ch.presence.is_empty() && ch.tx.receiver_count() == 0 {
                state.channels.remove(&self.post_id);
                None // nobody to notify
            } else {
                Some(count)
            }
        };
        if let Some(count) = new_count {
            self.hub
                .publish(self.post_id, LiveEvent::Presence { count });
        }
    }
}

/// Adapts a post's `BroadcastStream` into the SSE event stream and ties the
/// [`PresenceGuard`] to its lifetime. When the client disconnects axum drops the
/// response body, dropping this — `inner` (the receiver) first, then `_guard`,
/// so the guard's `receiver_count()` GC check sees the receiver already gone.
struct LiveStream {
    inner: BroadcastStream<LiveEvent>,
    _guard: PresenceGuard,
}

impl Stream for LiveStream {
    type Item = Result<Event, std::convert::Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            return match Pin::new(&mut self.inner).poll_next(cx) {
                Poll::Ready(Some(Ok(ev))) => {
                    let name = match &ev {
                        LiveEvent::Presence { .. } => "presence",
                        LiveEvent::Comment(_) => "comment",
                        LiveEvent::Reaction { .. } => "reaction",
                        LiveEvent::Data { .. } => "data",
                    };
                    match serde_json::to_string(&ev) {
                        Ok(json) => Poll::Ready(Some(Ok(Event::default().event(name).data(json)))),
                        // Serialization can't realistically fail for these types;
                        // if it ever did, skip the frame rather than kill the stream.
                        Err(_) => continue,
                    }
                }
                // A slow client fell behind the channel: skip the gap and keep
                // streaming. Absolute presence events resync it on the next tick.
                Poll::Ready(Some(Err(BroadcastStreamRecvError::Lagged(_)))) => continue,
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            };
        }
    }
}

/// `GET /api/live/{post_id}` — the SSE stream of live events for one post.
///
/// Public and anonymous (like `record_view`): presence is deduped by the coarse
/// header-derived `visitor_hash`, the only identity a browser `EventSource` can
/// convey (it can't set custom headers). Registered on the router in `main.rs`.
pub async fn live_handler(
    Path(post_id): Path<i64>,
    Extension(hub): HubExtension,
    headers: HeaderMap,
) -> impl axum::response::IntoResponse {
    let visitor = crate::server::analytics::visitor_hash(&headers);
    let (rx, guard, backlog) = hub.subscribe(post_id, visitor);
    // Replay the retained data points first (as the same `data` events live
    // points use), then transition to the live broadcast. The client's
    // `use_live` accumulates both identically, so the chart is backfilled with
    // no separate fetch and no client-side change.
    let history = backlog
        .into_iter()
        .filter_map(|ev| {
            serde_json::to_string(&ev).ok().map(|json| {
                Ok::<_, std::convert::Infallible>(Event::default().event("data").data(json))
            })
        })
        .collect::<Vec<_>>();
    let live = LiveStream {
        inner: BroadcastStream::new(rx),
        _guard: guard,
    };
    let stream = stream::iter(history).chain(live);
    // Keep-alive comments stop idle connections (and proxies) from timing out.
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
