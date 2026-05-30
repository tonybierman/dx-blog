//! Push a sample onto a post's live data series — the server-side source that
//! feeds the `livechart` embeds in the reader.
//!
//! This is the real-time twin of [`add_reaction`](crate::server::reactions):
//! it persists nothing, it just fans a [`LiveEvent::Data`](crate::model::LiveEvent)
//! out over the post's live channel so every connected reader's chart appends
//! the new point. A real deployment would call `hub.publish(post_id,
//! LiveEvent::Data { .. })` from wherever the data actually originates — a
//! background `tokio` task polling an upstream feed, a webhook handler, a metrics
//! tick — rather than from a public, anonymous server fn. This fn exists so the
//! whole pipeline is end-to-end testable and an author can drive a demo chart by
//! hand; gate or replace it before wiring a chart to anything sensitive.

use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::server::live::HubExtension;

/// Append `value` to a post's `topic` series for every reader currently watching.
#[post("/api/live-data/push", hub: HubExtension)]
pub async fn push_data_point(post_id: i64, topic: String, value: f64) -> Result<()> {
    let topic = topic.trim().to_string();
    if topic.is_empty() {
        return Err(ServerFnError::new("A data topic is required.").into());
    }
    hub.publish(post_id, crate::model::LiveEvent::Data { topic, value });
    Ok(())
}
