use dioxus::prelude::*;

/// Joins class fragments, dropping empties, into a single space-separated string.
fn join_classes(parts: &[&str]) -> String {
    parts
        .iter()
        .filter(|p| !p.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Bottom-margin override for the typography primitives. `Default` keeps each
/// component's own canonical spacing; the explicit variants *replace* it.
///
/// Margin is a prop (not an appended `class`) because Tailwind resolves
/// conflicting `mb-*` utilities by stylesheet source order, not by the order
/// they appear in the class attribute — so appending `mb-1` after a base
/// `mb-3` silently loses. Picking the class here emits exactly one `mb-*`.
#[derive(Copy, Clone, PartialEq, Default)]
pub enum Mb {
    #[default]
    Default,
    None,
    Mb1,
    Mb2,
    Mb3,
}

impl Mb {
    /// The explicit `mb-*` class, or `default` when left as `Default`.
    fn class_or(self, default: &'static str) -> &'static str {
        match self {
            Mb::Default => default,
            Mb::None => "",
            Mb::Mb1 => "mb-1",
            Mb::Mb2 => "mb-2",
            Mb::Mb3 => "mb-3",
        }
    }
}

/// `<h1 class="mb-6 text-2xl font-bold">` — the canonical page heading.
#[component]
pub fn PageTitle(
    /// Bottom margin; defaults to `mb-6`. Use `Mb::None` inside a header row
    /// whose wrapper already provides the spacing.
    #[props(default)]
    mb: Mb,
    /// Optional extra Tailwind classes (e.g. tracking overrides for hero variants).
    #[props(default)]
    class: String,
    children: Element,
) -> Element {
    let full = join_classes(&[mb.class_or("mb-6"), "text-2xl font-bold", &class]);
    rsx! { h1 { class: "{full}", {children} } }
}

#[derive(Copy, Clone, PartialEq, Default)]
pub enum SectionTitleTone {
    /// `<h2 class="mb-3 text-lg font-semibold">` — section inside a page.
    #[default]
    Default,
    /// `<h3 class="mb-2 font-semibold text-white/80">` — sidebar widget heading.
    Sidebar,
}

#[component]
pub fn SectionTitle(
    #[props(default)] tone: SectionTitleTone,
    /// Bottom margin; defaults to `mb-3` (Default tone) / `mb-2` (Sidebar tone).
    #[props(default)]
    mb: Mb,
    /// Optional extra Tailwind classes (e.g. `"mt-8"` to space from preceding content).
    #[props(default)]
    class: String,
    children: Element,
) -> Element {
    match tone {
        SectionTitleTone::Default => {
            let full = join_classes(&[mb.class_or("mb-3"), "text-lg font-semibold", &class]);
            rsx! { h2 { class: "{full}", {children} } }
        }
        SectionTitleTone::Sidebar => {
            let full = join_classes(&[mb.class_or("mb-2"), "font-semibold text-white/80", &class]);
            rsx! { h3 { class: "{full}", {children} } }
        }
    }
}

#[derive(Copy, Clone, PartialEq, Default)]
pub enum EyebrowAs {
    #[default]
    Span,
    Label,
    Div,
}

#[derive(Copy, Clone, PartialEq, Default)]
pub enum EyebrowSize {
    /// `text-xs` — the dominant eyebrow size.
    #[default]
    Xs,
    /// `text-sm` — larger eyebrow, e.g. an editor field label.
    Sm,
}

#[derive(Copy, Clone, PartialEq, Default)]
pub enum EyebrowTone {
    /// `text-white/40` — the dominant muted form.
    #[default]
    Muted,
    /// `text-brand-400` — for category eyebrows.
    Brand,
    /// `text-white/50` — slightly louder than Muted.
    Default,
}

impl EyebrowTone {
    fn class(self) -> &'static str {
        match self {
            EyebrowTone::Muted => "text-white/40",
            EyebrowTone::Brand => "text-brand-400",
            EyebrowTone::Default => "text-white/50",
        }
    }
}

/// `text-xs uppercase tracking-wide …` caps label.
#[component]
pub fn Eyebrow(
    #[props(default)] r#as: EyebrowAs,
    #[props(default)] tone: EyebrowTone,
    #[props(default)] size: EyebrowSize,
    /// Bottom margin; none by default.
    #[props(default)]
    mb: Mb,
    /// Optional extra Tailwind classes (e.g. `"block"` when used as a form label).
    #[props(default)]
    class: String,
    children: Element,
) -> Element {
    let size_class = match size {
        EyebrowSize::Xs => "text-xs",
        EyebrowSize::Sm => "text-sm",
    };
    let full = join_classes(&[
        mb.class_or(""),
        size_class,
        "uppercase tracking-wide",
        tone.class(),
        &class,
    ]);
    match r#as {
        EyebrowAs::Span => rsx! { span { class: "{full}", {children} } },
        EyebrowAs::Label => rsx! { label { class: "{full}", {children} } },
        EyebrowAs::Div => rsx! { div { class: "{full}", {children} } },
    }
}

/// `<p class="text-red-400">` — inline error message. Set `small` for `text-sm`,
/// `inline` to render a `<span>` (for errors sitting beside other inline content).
#[component]
pub fn ErrorText(
    #[props(default)] small: bool,
    #[props(default)] inline: bool,
    /// Optional extra Tailwind classes (e.g. `"mt-8"`).
    #[props(default)]
    class: String,
    children: Element,
) -> Element {
    let size = if small { "text-sm" } else { "" };
    let full = join_classes(&[size, "text-red-400", &class]);
    if inline {
        rsx! { span { class: "{full}", {children} } }
    } else {
        rsx! { p { class: "{full}", {children} } }
    }
}
