//! Server-side syntax highlighting for fenced code blocks.
//!
//! Mirrors `crate::server::render_markdown` (same pulldown-cmark options, same
//! ammonia sanitize) but routes fenced code through `syntect`, emitting
//! class-based `<span>`s paired with the committed `assets/highlight.css`
//! (base16-ocean.dark).
//!
//! Runs ONLY on the server. The reader receives the highlighted HTML as
//! serialized `crate::mdx::Segment`s via `get_post`, so syntect never ships to
//! the wasm client — the whole point of doing this here instead of inside the
//! shared `render_markdown`.

use std::sync::OnceLock;

use ammonia::Builder;
use pulldown_cmark::{html, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Class prefix on every highlight span (`syn-keyword`, `syn-string`, …),
/// matched by `assets/highlight.css`. Prefixed so the scope-name classes
/// syntect emits can't collide with Tailwind utilities elsewhere on the page.
const CLASS_PREFIX: &str = "syn-";

const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed {
    prefix: CLASS_PREFIX,
};

/// The bundled syntax definitions, loaded once. `load_defaults_newlines` matches
/// the `*_which_includes_newline` line API used below.
fn syntax_set() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Ammonia's `clean` strips `class` by default, which would delete the highlight
/// classes. This builder is the same default sanitizer but additionally permits
/// `class` on the three tags our highlighter emits — nothing else is loosened.
fn sanitizer() -> &'static Builder<'static> {
    static B: OnceLock<Builder<'static>> = OnceLock::new();
    B.get_or_init(|| {
        let mut b = Builder::default();
        // `data-lang` drives the CSS language label on `pre.syn` (highlight.css).
        b.add_tag_attributes("pre", ["class", "data-lang"]);
        b.add_tag_attributes("code", ["class"]);
        b.add_tag_attributes("span", ["class"]);
        b
    })
}

/// Render Markdown to sanitized HTML with syntax-highlighted code blocks.
/// Identical to `crate::server::render_markdown` for everything except fenced
/// code, which is replaced by syntect's class-based span markup.
pub fn render_markdown_highlighted(md: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);

    let ss = syntax_set();
    let parser = Parser::new_ext(md, options);

    // Buffer the text of the code block currently being read (lang token, source);
    // swap the whole block for highlighted HTML when it closes.
    let mut events: Vec<Event> = Vec::new();
    let mut code: Option<(String, String)> = None;

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split(|c: char| c == ',' || c.is_whitespace())
                        .next()
                        .unwrap_or("")
                        .to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                code = Some((lang, String::new()));
            }
            Event::Text(text) if code.is_some() => {
                code.as_mut().unwrap().1.push_str(&text);
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((lang, src)) = code.take() {
                    events.push(Event::Html(highlight_code(ss, &lang, &src).into()));
                }
            }
            other => events.push(other),
        }
    }

    let mut unsafe_html = String::new();
    html::push_html(&mut unsafe_html, events.into_iter());
    sanitizer().clean(&unsafe_html).to_string()
}

/// Highlight one code block into a `<pre class="syn"><code>…</code></pre>` with
/// class-based spans. Unknown/empty languages fall back to plain text; if syntect
/// errors mid-parse we emit an escaped plain block rather than dropping the code.
fn highlight_code(ss: &SyntaxSet, lang: &str, code: &str) -> String {
    let syntax = (!lang.is_empty())
        .then(|| ss.find_syntax_by_token(lang))
        .flatten()
        .unwrap_or_else(|| ss.find_syntax_plain_text());

    let mut generator = ClassedHTMLGenerator::new_with_class_style(syntax, ss, CLASS_STYLE);
    for line in LinesWithEndings::from(code) {
        if generator
            .parse_html_for_line_which_includes_newline(line)
            .is_err()
        {
            return plain_code_block(lang, code);
        }
    }
    wrap_pre(lang, &generator.finalize())
}

/// Escaped, unhighlighted fallback block (still sanitized downstream).
fn plain_code_block(lang: &str, code: &str) -> String {
    wrap_pre(lang, &escape_html(code))
}

/// Wrap highlighted/escaped `inner` in `<pre class="syn">`. When the fence named
/// a language, tag it with `data-lang` (CSS renders the corner label) and a
/// `language-*` class on the `<code>`. Both are attribute-escaped.
fn wrap_pre(lang: &str, inner: &str) -> String {
    if lang.is_empty() {
        format!("<pre class=\"syn\"><code>{inner}</code></pre>")
    } else {
        let lang = escape_attr(lang);
        format!(
            "<pre class=\"syn\" data-lang=\"{lang}\"><code class=\"language-{lang}\">{inner}</code></pre>"
        )
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(s: &str) -> String {
    escape_html(s).replace('"', "&quot;")
}

/// Like `crate::mdx::parse_body` but renders prose runs with highlighting. Used
/// by `get_post` to ship ready-to-display segments to the reader.
pub fn parse_body_highlighted(md: &str) -> Vec<crate::mdx::Segment> {
    crate::mdx::parse_body_with(md, render_markdown_highlighted)
}

/// The base16-ocean.dark theme rendered as a class-based stylesheet. Used by the
/// `generate_highlight_css` test to (re)produce the committed `assets/highlight.css`.
#[cfg(test)]
pub fn theme_css() -> String {
    use syntect::highlighting::ThemeSet;
    use syntect::html::css_for_theme_with_class_style;

    let ts = ThemeSet::load_defaults();
    let theme = &ts.themes["base16-ocean.dark"];
    css_for_theme_with_class_style(theme, CLASS_STYLE).expect("generate highlight css")
}

#[cfg(test)]
mod gen {
    use super::*;

    /// Regenerate `assets/highlight.css` from the bundled base16-ocean.dark theme.
    /// Ignored by default (it writes into the source tree); run on purpose with:
    /// `cargo test --features server,sqlite -- --ignored generate_highlight_css`
    #[test]
    #[ignore]
    fn generate_highlight_css() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/highlight.css");
        let header = "/* Generated by `server::highlight::gen::generate_highlight_css`\n   \
            (syntect base16-ocean.dark, class style `syn-`). Do not edit by hand. */\n";
        // Backstop styling for the highlighter's <pre class=\"syn\">: the theme's
        // editor background (so the block reads as one surface even where a token
        // has no explicit color), plus the corner language label fed by data-lang.
        let pre = "\npre.syn { background: #2b303b; color: #c0c5ce; position: relative; }\n\
            pre.syn[data-lang]::before {\n  \
            content: attr(data-lang);\n  \
            position: absolute;\n  top: 0;\n  right: 0;\n  \
            padding: 0.15em 0.6em;\n  \
            font-size: 0.72em;\n  \
            font-family: ui-monospace, SFMono-Regular, Menlo, monospace;\n  \
            color: #8a94a3;\n  background: #1f242d;\n  \
            border-bottom-left-radius: 0.5rem;\n  \
            text-transform: lowercase;\n  letter-spacing: 0.04em;\n  \
            pointer-events: none;\n}\n";
        std::fs::write(path, format!("{header}{}{pre}", theme_css())).unwrap();
    }
}
