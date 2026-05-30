# Live data & embedded charts

The reader page has a real-time layer: a presence badge ("N reading now"),
live-streamed comments, floating claps, and **live charts** embedded in post
bodies. They all ride the same mechanism — one Server-Sent Events (SSE) stream
per post, fed by an in-memory broadcast hub. This doc covers that mechanism and,
in particular, how the `livechart` embed turns server-pushed numbers into a
scrolling chart.

## The pieces

| Concern | Lives in |
|---|---|
| Wire event type (`LiveEvent`) | [`src/model/mod.rs`](../src/model/mod.rs) |
| The hub + SSE endpoint (server) | [`src/server/live.rs`](../src/server/live.rs) |
| The client hook (`use_live`) | [`src/live.rs`](../src/live.rs) |
| The chart embed (`LiveChart`) | [`src/embeds.rs`](../src/embeds.rs) |
| A demo data producer | [`src/main.rs`](../src/main.rs) |
| A manual push endpoint | [`src/server/live_data.rs`](../src/server/live_data.rs) |

## How a value reaches the chart

```
producer ──► hub.publish(post_id, LiveEvent::Data { topic, value })
                  │
                  ├─ append to the per-(post,topic) ring buffer   (history)
                  └─ broadcast to every connected reader of post_id
                                   │
                          GET /api/live/{post_id}  (SSE)
                                   │
                  use_live(): EventSource → data_points[topic]  (a signal)
                                   │
                  LiveChart (topic="…"): reads its topic, renders the SVG
```

1. **A producer publishes.** Anything server-side calls
   `hub.publish(post_id, LiveEvent::Data { topic, value })`. `topic` is a free
   string (`"cpu"`, `"mem"`, a sensor id…) so several charts on one page each
   follow their own series.
2. **The hub fans it out.** [`LiveHub`](../src/server/live.rs) holds one
   `tokio::sync::broadcast` channel per post. `publish` sends the event to every
   current subscriber **and** appends it to a retained ring buffer (see
   [Backfill](#backfill-late-joiners)).
3. **The SSE endpoint streams it.** `GET /api/live/{post_id}`
   (`live_handler`) is a plain axum streaming route — not a Dioxus server fn,
   because SSE needs a streaming response. Each `LiveEvent` goes out under a
   named SSE event (`presence` / `comment` / `reaction` / `data`).
4. **The client accumulates it.** [`use_live`](../src/live.rs) opens one
   browser `EventSource` per post, parses each frame back into a `LiveEvent`, and
   for `Data` events appends `value` to `data_points[topic]` — a
   `Signal<HashMap<String, Vec<f64>>>` capped per topic.
5. **The chart renders.** [`LiveChart`](../src/embeds.rs) reads its `topic` out
   of that signal and draws an SVG line, auto-scaled to the window's min/max.
   Reading the signal subscribes the component, so each pushed point re-renders
   it. **There is no client-side timer** — the chart is driven entirely by
   server pushes.

## Authoring a chart in a post

Embed a `livechart` block in the post body (Markdown):

```
[[component:livechart topic="cpu" window=28 color="#22d3ee" label="CPU %"]]
```

| Prop | Default | Meaning |
|---|---|---|
| `topic` | `live` | Which data series to subscribe to |
| `window` | `24` | Number of recent points to plot (clamped 2–200) |
| `color` | `#22d3ee` | Line/area color |
| `label` | `Live feed` | Caption shown above the chart |

Until the first point arrives (during SSR, before the `EventSource` connects, or
when nothing is feeding the topic), the chart shows a quiet "Waiting for live
data…" placeholder rather than an empty plot — the same SSR-safe discipline the
presence badge and reaction count follow.

The embed only *displays* data; something has to *produce* it for that
`post_id` + `topic`.

## Producing data

Three shapes, roughly in order of fit for a blog:

### 1. A background task (the demo)

[`src/main.rs`](../src/main.rs) spawns a task at startup that samples real host
**CPU** (`/proc/stat`) and **memory** (`/proc/meminfo`) every 2s and publishes
them on the `cpu` and `mem` topics of the seeded demo post:

```rust
let hub = Arc::clone(&hub);
let pool = producer_pool;
tokio::spawn(async move {
    // Resolve the post by its stable slug, not a literal id — the seed assigns
    // ids by insertion order, so a reseed could move it. Idle if it's absent.
    let post_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM posts WHERE slug = 'rust-mdx-livechart'")
            .fetch_optional(&pool).await.ok().flatten();
    let Some(post_id) = post_id else { return };

    let mut prev_cpu = None;
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    loop {
        tick.tick().await;
        for (topic, sample) in [
            ("cpu", cpu_percent(&mut prev_cpu).await),
            ("mem", mem_percent().await),
        ] {
            if let Some(value) = sample {
                hub.publish(post_id, LiveEvent::Data { topic: topic.into(), value });
            }
        }
    }
});
```

This is the canonical "live chart in a post" shape — swap the `/proc` readers
for whatever the post is about (a price API, GitHub stars, a metrics probe). The
metrics are **system-wide** and **Linux-only** (`/proc`); a failed sample just
skips that tick.

### 2. An ingest endpoint (push to us)

[`push_data_point`](../src/server/live_data.rs) is a server fn that publishes a
single point — useful when the data originates outside the server (a CI job, an
IoT device, a webhook). It's how the feature was first tested by hand:

```sh
curl -X POST http://127.0.0.1:8080/api/live-data/push \
  -H 'Content-Type: application/json' \
  -d '{"post_id":17,"topic":"cpu","value":42.0}'
```

> ⚠️ It's currently **anonymous** — anyone could drive any post's chart. Gate it
> (API key / shared secret) before relying on it in production.

### 3. Derived from existing events

The blog already records views and reactions. You could publish a rolling rate
("claps/min", "readers over time") by calling `hub.publish(.., LiveEvent::Data
{..})` from inside those server fns — no external source at all.

## Backfill (late joiners)

`broadcast` only reaches *currently connected* readers, so without help a reader
opening the page would see a blank chart until the next push. The hub keeps a
**ring buffer** of the most recent points per `(post_id, topic)`
(`HISTORY_MAX = 256`), retained even when no one is connected. On connect,
`live_handler` **replays that backlog** as ordinary `data` events before the
live stream takes over, so the chart paints with recent history immediately.

The backlog snapshot and the channel subscription happen under one mutex, so the
cut is exact: a point is either in the replayed backlog or delivered live —
never both, never neither.

## Constraints to know

- **Single process, in-memory.** The hub is one `Arc<LiveHub>` per process.
  Behind a load balancer with multiple instances, a reader on instance B won't
  get points published on instance A. Crossing that line needs a shared bus
  (Redis pub/sub, NATS, Postgres `LISTEN/NOTIFY`) behind `publish`.
- **History is retained per touched post.** The ring buffer is not garbage-
  collected when readers leave (that's what makes first-viewer backfill work).
  Fine for the handful of posts that host a chart; add GC if that changes.
- **The demo producer targets one post by slug.** It resolves
  `rust-mdx-livechart` at startup and idles if that post is absent (an unseeded
  DB). Reseeding works because the lookup is by slug, not a literal id. To drive
  a different/real post, change the slug (ideally lift it to config).
- **Producers run regardless of viewers.** Cheap for `/proc` reads; gate an
  expensive source on having subscribers.

## Adding a new metric

To add, say, disk usage:

1. Write a sampler (e.g. `disk_percent()` in `main.rs`).
2. Add `("disk", disk_percent().await)` to the producer loop.
3. Embed `[[component:livechart topic="disk" label="Disk %"]]` in the post.

No new component, event variant, or client code is needed — the path is
topic-generic end to end.
