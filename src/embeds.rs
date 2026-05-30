//! Registry of live, embeddable content blocks ("Rust MDX"). The markdown
//! renderer (`crate::mdx`) turns `[[component:name props…]]` lines into
//! [`Segment::Embed`](crate::mdx::Segment) values; [`EmbedBlock`] parses each
//! prop bag and dispatches to a real Dioxus component, so a post can host
//! interactive demos, charts, and tweakable visualizations instead of iframes.
//!
//! These are ordinary components (no feature gate): they render their initial
//! state during SSR and become interactive once the wasm client hydrates — the
//! whole reason for using components over an inert iframe. Props arrive as
//! strings; `EmbedBlock` reads only the keys each component understands and
//! falls back to sane defaults, so an author typo degrades gracefully rather
//! than breaking the page. Props are never injected as HTML, so this path is
//! strictly safer than an iframe embed.

use std::collections::BTreeMap;
use std::time::Duration;

use dioxus::prelude::*;
use dioxus_sdk_time::use_interval;

use crate::components::button::{Button, ButtonSize, ButtonVariant};

/// String prop bag parsed from an embed block.
type Props = BTreeMap<String, String>;

/// Look up a prop, returning a trimmed `&str` if present and non-empty.
fn prop<'a>(props: &'a Props, key: &str) -> Option<&'a str> {
    props.get(key).map(|s| s.trim()).filter(|s| !s.is_empty())
}

/// Look up a prop and parse it, falling back to `default` on absence/parse error.
fn prop_or<T: std::str::FromStr>(props: &Props, key: &str, default: T) -> T {
    prop(props, key)
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// Dispatch a parsed embed block to its component. Parsing the string prop bag
/// into typed props happens here so each component has a clean, typed surface.
/// Unknown names render a small inline notice rather than failing.
#[component]
pub fn EmbedBlock(name: String, props: Props) -> Element {
    rsx! {
        // `not-prose` opts the widget out of the article's Tailwind typography so
        // its own layout/spacing isn't overridden by `prose` rules.
        div { class: "not-prose my-6",
            match name.as_str() {
                "counter" => rsx! {
                    CounterDemo {
                        start: prop_or(&props, "start", 0),
                        step: prop_or(&props, "step", 1),
                        label: prop(&props, "label").unwrap_or("Counter").to_string(),
                    }
                },
                "chart" => rsx! {
                    InlineChart {
                        data: prop(&props, "data").unwrap_or("")
                            .split(',')
                            .filter_map(|s| s.trim().parse::<f64>().ok())
                            .collect::<Vec<f64>>(),
                        kind: prop(&props, "kind").unwrap_or("bar").to_string(),
                        color: prop(&props, "color").unwrap_or("#6366f1").to_string(),
                    }
                },
                "tweak" => rsx! {
                    TweakViz { label: prop(&props, "label").unwrap_or("Frequency").to_string() }
                },
                "livechart" => rsx! {
                    LiveChart {
                        topic: prop(&props, "topic").unwrap_or("live").to_string(),
                        window: prop_or(&props, "window", 24usize).clamp(2, 200),
                        color: prop(&props, "color").unwrap_or("#22d3ee").to_string(),
                        label: prop(&props, "label").unwrap_or("Live feed").to_string(),
                    }
                },
                "stockchart" | "ticker" => rsx! {
                    StockChart {
                        symbol: prop(&props, "symbol").unwrap_or("ACME").to_string(),
                        period_ms: prop_or(&props, "interval", 1000u64).max(200),
                        window: prop_or(&props, "window", 32usize).clamp(4, 200),
                        start: prop_or(&props, "start", 100.0_f64).max(1.0),
                    }
                },
                _ => rsx! { UnknownEmbed { name } },
            }
        }
    }
}

/// `[[component:counter start=0 step=1 label="..."]]` — a signal-driven counter
/// proving live reactivity inside an article.
#[component]
fn CounterDemo(start: i64, step: i64, label: String) -> Element {
    let mut count = use_signal(|| start);

    rsx! {
        div { class: "flex items-center gap-4 rounded-xl border border-white/10 bg-white/[0.03] p-4",
            Button {
                variant: ButtonVariant::Outline,
                size: ButtonSize::Icon,
                onclick: move |_| count -= step,
                "−"
            }
            div { class: "min-w-24 text-center",
                div { class: "text-2xl font-bold tabular-nums", "{count}" }
                div { class: "text-xs uppercase tracking-wide text-white/40", "{label}" }
            }
            Button {
                variant: ButtonVariant::Outline,
                size: ButtonSize::Icon,
                onclick: move |_| count += step,
                "+"
            }
        }
    }
}

/// `[[component:chart data="3,7,2,9" kind="bar|line" color="#..."]]` — a
/// hand-rolled SVG chart from inline data (no charting crate).
#[component]
fn InlineChart(data: Vec<f64>, kind: String, color: String) -> Element {
    if data.is_empty() {
        return rsx! {
            p { class: "rounded-lg border border-amber-400/30 bg-amber-400/10 px-3 py-2 text-sm text-amber-200",
                "chart embed: provide numeric data, e.g. ", code { "data=\"3,7,2,9\"" }
            }
        };
    }

    // Plot into a fixed 100×40 viewBox; the SVG scales to its container width.
    let (w, h, pad) = (100.0_f64, 40.0_f64, 2.0_f64);
    let max = data.iter().cloned().fold(f64::MIN, f64::max).max(1.0);
    let n = data.len();

    rsx! {
        div { class: "rounded-xl border border-white/10 bg-white/[0.03] p-4",
            svg {
                class: "h-40 w-full",
                view_box: "0 0 {w} {h}",
                preserve_aspect_ratio: "none",
                if kind == "line" {
                    polyline {
                        fill: "none",
                        stroke: "{color}",
                        stroke_width: "1",
                        points: line_points(&data, w, h, pad, max),
                    }
                } else {
                    {
                        let slot = w / n as f64;
                        let bar_w = slot * 0.7;
                        data.iter().enumerate().map(move |(i, v)| {
                            let bar_h = (v / max) * (h - pad * 2.0);
                            let x = i as f64 * slot + (slot - bar_w) / 2.0;
                            let y = h - pad - bar_h;
                            rsx! {
                                rect {
                                    key: "{i}",
                                    x: "{x}", y: "{y}",
                                    width: "{bar_w}", height: "{bar_h}",
                                    rx: "0.6", fill: "{color}",
                                }
                            }
                        })
                    }
                }
            }
        }
    }
}

/// Build the `points` attribute for a polyline spanning the viewBox.
fn line_points(data: &[f64], w: f64, h: f64, pad: f64, max: f64) -> String {
    let n = data.len();
    let step = if n > 1 { w / (n - 1) as f64 } else { 0.0 };
    data.iter()
        .enumerate()
        .map(|(i, v)| {
            let x = i as f64 * step;
            let y = h - pad - (v / max) * (h - pad * 2.0);
            format!("{x:.2},{y:.2}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `[[component:tweak label="..."]]` — a slider that drives a computed SVG sine
/// curve live, demonstrating in-browser WASM compute reacting to input.
#[component]
fn TweakViz(label: String) -> Element {
    let mut freq = use_signal(|| 3.0_f64);

    // Recompute the curve from the current frequency on every change.
    let points = use_memo(move || {
        let f = freq();
        (0..=100)
            .map(|i| {
                let x = i as f64;
                let t = x / 100.0 * std::f64::consts::TAU * f;
                let y = 20.0 - t.sin() * 16.0;
                format!("{x:.1},{y:.2}")
            })
            .collect::<Vec<_>>()
            .join(" ")
    });

    rsx! {
        div { class: "rounded-xl border border-white/10 bg-white/[0.03] p-4",
            svg {
                class: "h-32 w-full",
                view_box: "0 0 100 40",
                preserve_aspect_ratio: "none",
                polyline {
                    fill: "none",
                    stroke: "#22d3ee",
                    stroke_width: "1",
                    points: "{points}",
                }
            }
            div { class: "mt-3 flex items-center gap-3 text-sm",
                span { class: "w-24 text-white/50", "{label}" }
                input {
                    r#type: "range",
                    class: "flex-1 accent-brand-500",
                    min: "1", max: "12", step: "0.5",
                    value: "{freq}",
                    oninput: move |e| {
                        if let Ok(v) = e.value().parse::<f64>() {
                            freq.set(v);
                        }
                    },
                }
                span { class: "w-10 text-right tabular-nums text-white/70", "{freq}" }
            }
        }
    }
}

/// `[[component:livechart topic="cpu" window=24 color="#..." label="..."]]` — a
/// chart fed by the post's live (SSE) channel. Unlike the other embeds it runs
/// no timer and synthesizes no data: it subscribes to the shared
/// [`LiveHandle`](crate::live::LiveHandle) the reader provides via context and
/// renders the `topic` series, appending each `LiveEvent::Data { topic, value }`
/// the server pushes. This is the same real-time path reactions, comments, and
/// presence ride — so charts get true server-driven data rather than a
/// client-side simulation, and several charts on one page each track their own
/// `topic`.
///
/// Rendered outside a reader (e.g. an admin preview) there's no provider, so the
/// handle is absent and the chart shows an idle placeholder rather than failing.
/// During SSR — and until the first point arrives — the series is empty, so the
/// server HTML and the client's first paint agree (same discipline the presence
/// badge and reaction count already follow).
#[component]
fn LiveChart(topic: String, window: usize, color: String, label: String) -> Element {
    // Subscribe to this post's live data via the handle the reader provided.
    // `None` when rendered without a provider — degrade to an idle chart then.
    let live = try_use_context::<crate::live::LiveHandle>();

    // The newest `window` points of our topic. Reading the signal here
    // subscribes this component, so a pushed `Data` event re-renders the chart.
    let series: Vec<f64> = live
        .map(|h| {
            let all = (h.data_points)();
            let s = all.get(&topic).cloned().unwrap_or_default();
            let start = s.len().saturating_sub(window);
            s[start..].to_vec()
        })
        .unwrap_or_default();

    // Nothing yet (SSR, no provider, or feed not started): show a quiet idle
    // state instead of an empty/garbage plot.
    if series.is_empty() {
        return rsx! {
            div { class: "rounded-xl border border-white/10 bg-white/[0.03] p-4",
                div { class: "mb-2 flex items-center gap-2 text-sm",
                    span { class: "inline-block h-2 w-2 animate-pulse rounded-full bg-white/30" }
                    span { class: "text-xs uppercase tracking-wide text-white/40", "{label}" }
                }
                div { class: "flex h-40 items-center justify-center text-xs text-white/30",
                    "Waiting for live data…"
                }
            }
        };
    }

    let latest = series.last().copied().unwrap_or(0.0);

    // Plot into a fixed 100×40 viewBox; the SVG scales to its container width.
    // Real data has an unknown range, so auto-scale to the window's min/max
    // (like StockChart) rather than assuming a fixed 0–100 domain.
    let (w, h, pad) = (100.0_f64, 40.0_f64, 2.0_f64);
    let n = series.len();
    let dx = if n > 1 { w / (n - 1) as f64 } else { 0.0 };
    let lo = series.iter().cloned().fold(f64::MAX, f64::min);
    let hi = series.iter().cloned().fold(f64::MIN, f64::max);
    let range = (hi - lo).max(1e-6);
    let span = h - pad * 2.0;
    let xy = |i: usize, v: f64| {
        let x = i as f64 * dx;
        let y = h - pad - (v - lo) / range * span;
        (x, y)
    };
    let line = series
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let (x, y) = xy(i, v);
            format!("{x:.2},{y:.2}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    // Close the polyline down to the baseline for a soft area fill.
    let area = format!("0,{:.2} {line} {:.2},{:.2}", h, w, h);
    let (head_x, head_y) = xy(n.saturating_sub(1), latest);

    rsx! {
        div { class: "rounded-xl border border-white/10 bg-white/[0.03] p-4",
            div { class: "mb-2 flex items-center justify-between text-sm",
                div { class: "flex items-center gap-2",
                    span { class: "inline-block h-2 w-2 animate-pulse rounded-full bg-red-500" }
                    span { class: "text-xs uppercase tracking-wide text-white/50", "{label}" }
                }
                span { class: "tabular-nums font-semibold", "{latest:.1}" }
            }
            svg {
                class: "h-40 w-full",
                view_box: "0 0 {w} {h}",
                preserve_aspect_ratio: "none",
                polygon { points: "{area}", fill: "{color}", opacity: "0.12" }
                polyline {
                    fill: "none",
                    stroke: "{color}",
                    stroke_width: "1",
                    stroke_linejoin: "round",
                    points: "{line}",
                }
                circle { cx: "{head_x}", cy: "{head_y}", r: "1.1", fill: "{color}" }
            }
        }
    }
}

/// One OHLC candle plus its (synthetic) traded volume.
#[derive(Clone, PartialEq)]
struct Candle {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

/// `[[component:stockchart symbol="ACME" interval=1000 window=32 start=100]]`
/// (alias `ticker`) — a candlestick chart that looks like it's moving during
/// open trading: every `interval` ms a fresh candle prints on the right, the
/// price ticks, and the session change flips green/red.
///
/// Prices come from a deterministic hashed random-walk (no `rand`/clock), so the
/// opening window is identical on the server and the client's first paint;
/// trading only "opens" once `use_interval` is polled after hydration.
#[component]
fn StockChart(symbol: String, period_ms: u64, window: usize, start: f64) -> Element {
    // Seed an opening session by walking `window` candles from the start price.
    let mut step = use_signal(|| window as u64);
    let mut candles = use_signal(|| {
        let mut v = Vec::with_capacity(window);
        let mut open = start;
        for s in 0..window as u64 {
            let c = next_candle(open, s);
            open = c.close;
            v.push(c);
        }
        v
    });

    use_interval(Duration::from_millis(period_ms), move |()| {
        let s = step() + 1;
        step.set(s);
        let mut cs = candles();
        let open = cs.last().map(|c| c.close).unwrap_or(start);
        cs.push(next_candle(open, s));
        let overflow = cs.len().saturating_sub(window);
        if overflow > 0 {
            cs.drain(0..overflow);
        }
        candles.set(cs);
    });

    let series = candles();
    let session_open = series.first().map(|c| c.open).unwrap_or(start);
    let last = series.last().map(|c| c.close).unwrap_or(start);
    let change = last - session_open;
    let pct = change / session_open * 100.0;
    let up = change >= 0.0;
    let (up_color, down_color) = ("#34d399", "#f87171");
    let accent = if up { up_color } else { down_color };
    let arrow = if up { "▲" } else { "▼" };
    let sign = if up { "+" } else { "" };

    // 100×48 viewBox split into a price band (candles) and a volume band below.
    let (w, h, pad) = (100.0_f64, 48.0_f64, 2.0_f64);
    let vol_h = 10.0_f64; // height of the volume sub-chart
    let gap = 2.0_f64; // gap between price and volume bands
    let (price_top, price_bot) = (pad, h - pad - vol_h - gap);
    let vol_bot = h - pad;

    let lo = series.iter().map(|c| c.low).fold(f64::MAX, f64::min);
    let hi = series.iter().map(|c| c.high).fold(f64::MIN, f64::max);
    let range = (hi - lo).max(1e-6);
    let price_span = price_bot - price_top;
    let y = move |p: f64| price_bot - (p - lo) / range * price_span;

    let vol_max = series
        .iter()
        .map(|c| c.volume)
        .fold(0.0_f64, f64::max)
        .max(1e-6);

    let n = series.len();
    let slot = w / n as f64;
    let body_w = (slot * 0.6).max(0.4);

    rsx! {
        div { class: "rounded-xl border border-white/10 bg-white/[0.03] p-4",
            div { class: "mb-2 flex items-end justify-between",
                div { class: "flex items-center gap-2",
                    span { class: "font-semibold tracking-wide", "{symbol}" }
                    span { class: "flex items-center gap-1 text-[10px] uppercase tracking-wide text-emerald-400",
                        span { class: "inline-block h-1.5 w-1.5 animate-pulse rounded-full bg-emerald-400" }
                        "Market open"
                    }
                }
                div { class: "text-right",
                    div { class: "tabular-nums text-lg font-bold", "{last:.2}" }
                    div { class: "text-xs tabular-nums", style: "color: {accent}",
                        "{arrow} {sign}{change:.2} ({sign}{pct:.2}%)"
                    }
                }
            }
            svg {
                class: "h-44 w-full",
                view_box: "0 0 {w} {h}",
                preserve_aspect_ratio: "none",
                {
                    series.iter().enumerate().map(move |(i, c)| {
                        let cx = i as f64 * slot + slot / 2.0;
                        let color = if c.close >= c.open { up_color } else { down_color };
                        let top = c.open.max(c.close);
                        let bot = c.open.min(c.close);
                        let (y_top, y_bot) = (y(top), y(bot));
                        let body_h = (y_bot - y_top).max(0.3);
                        let x = cx - body_w / 2.0;
                        let (y_high, y_low) = (y(c.high), y(c.low));
                        // Volume bar grows up from the band baseline.
                        let vbar_h = (c.volume / vol_max * vol_h).max(0.2);
                        let vy = vol_bot - vbar_h;
                        rsx! {
                            g { key: "{i}",
                                line {
                                    x1: "{cx}", y1: "{y_high}",
                                    x2: "{cx}", y2: "{y_low}",
                                    stroke: "{color}", stroke_width: "0.3",
                                }
                                rect {
                                    x: "{x}", y: "{y_top}",
                                    width: "{body_w}", height: "{body_h}",
                                    fill: "{color}",
                                }
                                rect {
                                    x: "{x}", y: "{vy}",
                                    width: "{body_w}", height: "{vbar_h}",
                                    fill: "{color}", opacity: "0.45",
                                }
                            }
                        }
                    })
                }
            }
        }
    }
}

/// Deterministic next OHLC candle from the prior `open`, hashed off `step` so the
/// walk is reproducible (keeps SSR and first client render in sync).
fn next_candle(open: f64, step: u64) -> Candle {
    let drift = (rnd(step, 1) - 0.5) * 2.0; // [-1, 1]
    let close = (open * (1.0 + drift * 0.012)).max(1.0);
    let high = open.max(close) * (1.0 + rnd(step, 2) * 0.008);
    let low = open.min(close) * (1.0 - rnd(step, 3) * 0.008);
    // Volume loosely tracks the size of the move, plus a noise floor.
    let volume = 0.3 + rnd(step, 4) * 0.7 + drift.abs() * 0.6;
    Candle {
        open,
        high,
        low,
        close,
        volume,
    }
}

/// Cheap hashed pseudo-random in `[0, 1)` from `(step, salt)` — no `rand` crate.
fn rnd(step: u64, salt: u64) -> f64 {
    let mut x = step
        .wrapping_mul(2_654_435_761)
        .wrapping_add(salt.wrapping_mul(40_503));
    x ^= x >> 13;
    x = x.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^= x >> 7;
    (x % 1_000_000) as f64 / 1_000_000.0
}

/// Fallback for an unrecognized component name.
#[component]
fn UnknownEmbed(name: String) -> Element {
    rsx! {
        p { class: "rounded-lg border border-amber-400/30 bg-amber-400/10 px-3 py-2 text-sm text-amber-200",
            "Unknown embed component: ", code { "{name}" }
        }
    }
}
