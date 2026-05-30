//! "Rust MDX": split a post body into a sequence of rendered-markdown runs and
//! embed blocks so the reader can mount real, interactive Dioxus components
//! (charts, demos, tweakable visualizations) instead of iframes.
//!
//! An embed is a standalone line of the form
//!
//! ```text
//! [[component:name key=value key2="quoted value"]]
//! ```
//!
//! Everything else accumulates into markdown runs that are rendered through the
//! same `render_markdown` (pulldown-cmark + ammonia) pipeline the server uses to
//! produce stored `body_html`, so prose between embeds is byte-for-byte what a
//! plain post would render.
//!
//! Compiled for both the server (SSR) and the wasm client (hydration + the
//! editor's live preview), mirroring `crate::server::render_markdown`.
//!
//! Limitation: markdown reference-style links and footnote definitions must live
//! in the *same* run as their use — each run is rendered independently, so a
//! definition on the far side of an embed block won't resolve.

#![cfg(any(feature = "server", feature = "web"))]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One piece of a parsed post body, in document order.
///
/// `Serialize`/`Deserialize` so the reader can render segments the server
/// produced (with syntax-highlighted code) instead of re-running the markdown
/// pipeline on the client — see `crate::server::highlight::parse_body_highlighted`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Segment {
    /// A run of ordinary markdown, already rendered to sanitized HTML.
    Html(String),
    /// An embed block to be mounted as a live component.
    Embed {
        name: String,
        props: BTreeMap<String, String>,
    },
}

/// Split `md` into HTML runs and embed blocks, rendering each prose run with the
/// shared `crate::server::render_markdown` (pulldown-cmark + ammonia). A body
/// with no embeds yields a single `Html` segment identical to `render_markdown(md)`.
pub fn parse_body(md: &str) -> Vec<Segment> {
    parse_body_with(md, crate::server::render_markdown)
}

/// `parse_body` with a pluggable prose renderer. The embed grammar is identical;
/// only the function that turns each markdown run into HTML differs. The server
/// passes a syntax-highlighting renderer here (see `server::highlight`), while
/// `parse_body` passes the plain pipeline shared with the editor's live preview.
pub fn parse_body_with(md: &str, render: impl Fn(&str) -> String) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut run = String::new();

    let flush = |run: &mut String, segments: &mut Vec<Segment>| {
        if !run.trim().is_empty() {
            segments.push(Segment::Html(render(run)));
        }
        run.clear();
    };

    for line in md.lines() {
        match parse_embed_line(line) {
            Some((name, props)) => {
                flush(&mut run, &mut segments);
                segments.push(Segment::Embed { name, props });
            }
            None => {
                run.push_str(line);
                run.push('\n');
            }
        }
    }
    flush(&mut run, &mut segments);
    segments
}

/// Recognize a standalone `[[component:name props…]]` line, returning the
/// component name and its parsed props. Returns `None` for any other line.
fn parse_embed_line(line: &str) -> Option<(String, BTreeMap<String, String>)> {
    let inner = line.trim().strip_prefix("[[")?.strip_suffix("]]")?.trim();
    let rest = inner.strip_prefix("component:")?.trim_start();

    // The name runs up to the first whitespace; the remainder is props.
    let (name, props_str) = match rest.find(char::is_whitespace) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    Some((name.to_string(), parse_props(props_str)))
}

/// Tokenize `key=value` and `key="quoted value"` pairs. Bare flags (`key` with
/// no `=`) map to an empty string. Unknown keys are kept; components read only
/// what they recognize.
fn parse_props(s: &str) -> BTreeMap<String, String> {
    let mut props = BTreeMap::new();
    let mut chars = s.chars().peekable();

    loop {
        // Skip whitespace between tokens.
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        if chars.peek().is_none() {
            break;
        }

        // Read the key (up to '=' or whitespace).
        let mut key = String::new();
        while let Some(&c) = chars.peek() {
            if c == '=' || c.is_whitespace() {
                break;
            }
            key.push(c);
            chars.next();
        }

        // Optional `=value`.
        let mut value = String::new();
        if chars.peek() == Some(&'=') {
            chars.next(); // consume '='
            if chars.peek() == Some(&'"') {
                chars.next(); // consume opening quote
                for c in chars.by_ref() {
                    if c == '"' {
                        break;
                    }
                    value.push(c);
                }
            } else {
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() {
                        break;
                    }
                    value.push(c);
                    chars.next();
                }
            }
        }

        if !key.is_empty() {
            props.insert(key, value);
        }
    }
    props
}
