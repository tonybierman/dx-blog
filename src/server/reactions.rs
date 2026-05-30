//! Anonymous reactions ("claps") — add one, or read a post's running total.
//!
//! Like view tracking, reactions need no account: a clap is one row keyed by the
//! coarse [`visitor_hash`](crate::server::analytics::visitor_hash). Each
//! successful clap is fanned out over the post's live channel
//! ([`crate::server::live`]) so other readers see it float in real time.

use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::db::reactions::{
    insert_reaction_db, post_is_published_db, reaction_burst_count_db, reaction_total_db,
    reaction_visitor_total_db,
};
#[cfg(feature = "server")]
use crate::server::{live::HubExtension, sfe, DbExtension};

/// Reaction kinds the server will store. Bounding this keeps `kind` from being a
/// free-text column an anonymous caller could stuff arbitrary data into; the
/// registry can grow as the UI gains more reactions.
#[cfg(feature = "server")]
const ALLOWED_KINDS: [&str; 1] = ["clap"];

/// Per-visitor-per-post lifetime cap. A reader can cheer enthusiastically, but
/// not mint unbounded rows; mirrors the spirit of the comment anti-flood gates.
#[cfg(feature = "server")]
const MAX_PER_VISITOR: i64 = 50;

/// Burst window + cap: a single visitor can fire at most [`BURST_MAX`] claps
/// within this trailing span on one post, so a held-down button can't flood.
#[cfg(feature = "server")]
const BURST_WINDOW: &str = "-10 seconds";
#[cfg(feature = "server")]
const BURST_MAX: i64 = 15;

/// Record one reaction for a post and broadcast it live. Public + anonymous.
///
/// Validates the post is published (so arbitrary/draft ids can't accrue rows,
/// matching `record_view`/`create_comment`), throttles per visitor, then inserts
/// and publishes a [`LiveEvent::Reaction`](crate::model::LiveEvent). Errors are
/// mapped through `sfe` so nothing about the schema leaks.
#[post("/api/reactions/add", db: DbExtension, hub: HubExtension, headers: axum::http::HeaderMap)]
pub async fn add_reaction(post_id: i64, kind: String) -> Result<()> {
    let kind = kind.trim().to_string();
    if !ALLOWED_KINDS.contains(&kind.as_str()) {
        return Err(ServerFnError::new("Unknown reaction.").into());
    }

    if !post_is_published_db(&db.0, post_id).await.map_err(sfe)? {
        return Err(ServerFnError::new("Post not found.").into());
    }

    let visitor = crate::server::analytics::visitor_hash(&headers);

    let total = reaction_visitor_total_db(&db.0, post_id, &visitor)
        .await
        .map_err(sfe)?;
    if total >= MAX_PER_VISITOR {
        // Already a generous number of claps from this visitor — quietly succeed
        // so the UI doesn't surface an error for an over-eager reader.
        return Ok(());
    }

    let burst = reaction_burst_count_db(&db.0, post_id, &visitor, BURST_WINDOW)
        .await
        .map_err(sfe)?;
    if burst >= BURST_MAX {
        return Err(ServerFnError::new("You're clapping too fast — give it a sec.").into());
    }

    insert_reaction_db(&db.0, post_id, &kind, &visitor)
        .await
        .map_err(sfe)?;

    // Broadcast the authoritative post-insert total so every client shows the
    // same number rather than tracking its own increments.
    let total = reaction_total_db(&db.0, post_id).await.map_err(sfe)?;
    hub.publish(post_id, crate::model::LiveEvent::Reaction { kind, total });
    Ok(())
}

/// Total reactions a post has accumulated — the initial number the live badge
/// starts from before SSE increments take over. Public.
#[post("/api/reactions/total", db: DbExtension)]
pub async fn reaction_total(post_id: i64) -> Result<i64> {
    Ok(reaction_total_db(&db.0, post_id).await.map_err(sfe)?)
}
