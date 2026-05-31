use dioxus::prelude::*;

#[derive(Copy, Clone, PartialEq, Default)]
pub enum PanelVariant {
    /// `rounded-xl border-white/10 bg-white/[0.03]` — filled marketing/dashboard surface.
    #[default]
    Filled,
    /// `rounded-lg border-white/10` — outlined list-row card, no fill.
    Outlined,
    /// `rounded-xl border-white/10 bg-white/[0.03] overflow-hidden` — filled with image bleed,
    /// internal padding is the caller's responsibility.
    Bleed,
}

#[derive(Copy, Clone, PartialEq, Default)]
pub enum PanelPadding {
    None,
    Sm,
    Md,
    #[default]
    Lg,
}

impl PanelPadding {
    fn class(self) -> &'static str {
        match self {
            PanelPadding::None => "",
            PanelPadding::Sm => "p-2",
            PanelPadding::Md => "p-3",
            PanelPadding::Lg => "p-4",
        }
    }
}

/// Returns the class string for the panel variant + padding combination.
/// Use this for call sites that need a non-`<div>` element (e.g. `<article>`).
pub fn panel_class(variant: PanelVariant, padding: PanelPadding) -> String {
    let base = match variant {
        PanelVariant::Filled => "rounded-xl border border-white/10 bg-white/[0.03]",
        PanelVariant::Outlined => "rounded-lg border border-white/10",
        PanelVariant::Bleed => "overflow-hidden rounded-xl border border-white/10 bg-white/[0.03]",
    };
    let pad = padding.class();
    if pad.is_empty() {
        base.to_string()
    } else {
        format!("{base} {pad}")
    }
}

#[component]
pub fn Panel(
    #[props(default)] variant: PanelVariant,
    #[props(default)] padding: PanelPadding,
    /// Optional extra Tailwind classes appended after the base panel classes
    /// (e.g. `"flex items-center gap-4"`).
    #[props(default)]
    class: String,
    children: Element,
) -> Element {
    let base = panel_class(variant, padding);
    let full = if class.is_empty() {
        base
    } else {
        format!("{base} {class}")
    };
    rsx! { div { class: "{full}", {children} } }
}

// Full semantic palette — not every tone is used at a call site yet.
#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq, Default)]
pub enum AlertTone {
    #[default]
    Warning,
    Info,
    Success,
    Danger,
}

impl AlertTone {
    fn class(self) -> &'static str {
        match self {
            AlertTone::Warning => "border-amber-400/30 bg-amber-400/10 text-amber-200",
            AlertTone::Info => "border-sky-400/30 bg-sky-400/10 text-sky-200",
            AlertTone::Success => "border-emerald-400/30 bg-emerald-400/10 text-emerald-200",
            AlertTone::Danger => "border-red-400/30 bg-red-400/10 text-red-200",
        }
    }
}

#[component]
pub fn Alert(
    #[props(default)] tone: AlertTone,
    /// Optional extra Tailwind classes appended after the base alert classes
    /// (e.g. `"mb-6"`).
    #[props(default)]
    class: String,
    children: Element,
) -> Element {
    let tone_class = tone.class();
    let full = if class.is_empty() {
        format!("rounded-lg border px-3 py-2 text-sm {tone_class}")
    } else {
        format!("rounded-lg border px-3 py-2 text-sm {tone_class} {class}")
    };
    rsx! { p { class: "{full}", {children} } }
}

// Full semantic palette: some tones aren't used at a call site yet, but the
// complete set is what keeps the badge vocabulary consistent and grep-able
// (mirrors AlertTone above, which carries the same not-all-used tone set).
#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq, Default)]
pub enum BadgeTone {
    #[default]
    Brand,
    Amber,
    Emerald,
    Red,
    Neutral,
}

impl BadgeTone {
    fn tinted_class(self) -> &'static str {
        match self {
            BadgeTone::Brand => "bg-brand-500/10 text-brand-300",
            BadgeTone::Amber => "bg-amber-400/10 text-amber-300",
            BadgeTone::Emerald => "bg-emerald-400/10 text-emerald-300",
            BadgeTone::Red => "bg-red-400/10 text-red-300",
            BadgeTone::Neutral => "bg-white/[0.06] text-white/70",
        }
    }
    fn outlined_class(self) -> &'static str {
        match self {
            BadgeTone::Brand => "border border-brand-400/40 text-brand-300",
            BadgeTone::Amber => "border border-amber-400/40 text-amber-300",
            BadgeTone::Emerald => "border border-emerald-400/40 text-emerald-300",
            BadgeTone::Red => "border border-red-400/40 text-red-300",
            BadgeTone::Neutral => "border border-white/15 text-white/70",
        }
    }
    fn dot_class(self) -> &'static str {
        match self {
            BadgeTone::Brand => "bg-brand-400",
            BadgeTone::Amber => "bg-amber-400",
            BadgeTone::Emerald => "bg-emerald-400",
            BadgeTone::Red => "bg-red-400",
            BadgeTone::Neutral => "bg-white/40",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Default)]
pub enum BadgeVariant {
    #[default]
    Tinted,
    Outlined,
}

#[component]
pub fn Badge(
    #[props(default)] tone: BadgeTone,
    #[props(default)] variant: BadgeVariant,
    /// Prepends a pulsing dot in the same tone.
    #[props(default)]
    dot: bool,
    children: Element,
) -> Element {
    let tone_classes = match variant {
        BadgeVariant::Tinted => tone.tinted_class(),
        BadgeVariant::Outlined => tone.outlined_class(),
    };
    // inline-flex is needed when a dot is present; harmless otherwise.
    let full =
        format!("inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs {tone_classes}");
    let dot_class = tone.dot_class();
    rsx! {
        span { class: "{full}",
            if dot {
                span { class: "h-1.5 w-1.5 animate-pulse rounded-full {dot_class}" }
            }
            {children}
        }
    }
}

// Full semantic palette — see BadgeTone above; mirrored for consistency.
#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq, Default)]
pub enum StatusDotTone {
    #[default]
    Brand,
    Amber,
    Emerald,
    Red,
    Neutral,
}

impl StatusDotTone {
    fn class(self) -> &'static str {
        match self {
            StatusDotTone::Brand => "bg-brand-400",
            StatusDotTone::Amber => "bg-amber-400",
            StatusDotTone::Emerald => "bg-emerald-400",
            StatusDotTone::Red => "bg-red-500",
            StatusDotTone::Neutral => "bg-white/30",
        }
    }
}

#[component]
pub fn StatusDot(
    #[props(default)] tone: StatusDotTone,
    /// Renders the smaller 1.5×1.5 dot instead of the default 2×2.
    #[props(default)]
    muted: bool,
    #[props(default = true)] pulse: bool,
) -> Element {
    let size = if muted { "h-1.5 w-1.5" } else { "h-2 w-2" };
    let tone_class = tone.class();
    let pulse_class = if pulse { "animate-pulse" } else { "" };
    rsx! {
        span { class: "inline-block {size} rounded-full {pulse_class} {tone_class}" }
    }
}
