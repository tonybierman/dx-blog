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

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::extract::{Extension, Path};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::Stream;
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

/// One post's fan-out channel plus its live-reader tally.
struct PostChannel {
    tx: broadcast::Sender<LiveEvent>,
    /// `visitor_hash` -> number of open connections from that visitor. "Reading
    /// now" is the number of *distinct* visitors, i.e. `presence.len()`, so two
    /// tabs from one browser count once.
    presence: HashMap<String, u32>,
}

/// In-memory registry of per-post live channels. Cheap to clone (it's always
/// behind an `Arc`); all state is guarded by one `Mutex`.
#[derive(Default)]
pub struct LiveHub {
    posts: Mutex<HashMap<i64, PostChannel>>,
}

impl LiveHub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Subscribe to a post's events and register one unit of presence. Returns
    /// the receiver to stream from and a guard whose `Drop` removes the presence
    /// again. The current (post-increment) reader count is broadcast so the
    /// just-joined client — and everyone already reading — sees it immediately.
    pub fn subscribe(
        self: &Arc<Self>,
        post_id: i64,
        visitor: String,
    ) -> (broadcast::Receiver<LiveEvent>, PresenceGuard) {
        let (rx, count) = {
            let mut posts = self.posts.lock().unwrap();
            let ch = posts.entry(post_id).or_insert_with(|| PostChannel {
                tx: broadcast::channel(CHANNEL_CAP).0,
                presence: HashMap::new(),
            });
            let rx = ch.tx.subscribe();
            *ch.presence.entry(visitor.clone()).or_insert(0) += 1;
            (rx, ch.presence.len() as i64)
        }; // lock released before publish() re-locks — never held across the send.
        self.publish(post_id, LiveEvent::Presence { count });
        (
            rx,
            PresenceGuard {
                hub: Arc::clone(self),
                post_id,
                visitor,
            },
        )
    }

    /// Fan an event out to every subscriber of `post_id`. A no-op if the post has
    /// no channel or no receivers (`send` only errors when there are none).
    pub fn publish(&self, post_id: i64, event: LiveEvent) {
        let posts = self.posts.lock().unwrap();
        if let Some(ch) = posts.get(&post_id) {
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
            let mut posts = self.hub.posts.lock().unwrap();
            let Some(ch) = posts.get_mut(&self.post_id) else {
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
            // order in `LiveStream`.
            if ch.presence.is_empty() && ch.tx.receiver_count() == 0 {
                posts.remove(&self.post_id);
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
    let (rx, guard) = hub.subscribe(post_id, visitor);
    let stream = LiveStream {
        inner: BroadcastStream::new(rx),
        _guard: guard,
    };
    // Keep-alive comments stop idle connections (and proxies) from timing out.
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
