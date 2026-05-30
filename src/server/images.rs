//! Image rendition pipeline (server-only).
//!
//! WordPress generates a fixed set of resized copies of every uploaded image and
//! serves the smallest one that fits each slot. We do the same, but modern-format
//! first: on upload we decode the image once, resize it to a small set of widths
//! (never upscaling), and re-encode each as WebP — and, when the `avif` feature is
//! compiled, AVIF too. The originals stay the canonical file; these renditions are
//! the responsive `srcset` sources.
//!
//! This module is the pure CPU half (decode/resize/encode + the render-time
//! `<img>`→`<picture>` rewrite). Persisting renditions and building `srcset`
//! strings lives in `crate::db::media`; the upload/backfill orchestration that
//! calls both lives in `crate::server::admin::media` / `crate::main`.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;

/// One generated rendition, ready to write to disk and record in `media_variants`.
pub struct GeneratedVariant {
    /// Size bucket: `thumb` / `small` / `medium` / `large` / `full`.
    pub label: &'static str,
    /// Codec: `webp` or `avif`.
    pub format: &'static str,
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

/// Target widths, mirroring WordPress's thumbnail/medium/large idea. Only those
/// strictly narrower than the original are produced; the original width is added
/// separately as `full` so even the largest slot gets a modern-format option.
const TARGET_WIDTHS: &[(&str, u32)] = &[
    ("thumb", 320),
    ("small", 640),
    ("medium", 1024),
    ("large", 1600),
];

/// WebP quality (0–100). 80 is a good size/quality knee for photos.
const WEBP_QUALITY: f32 = 80.0;

/// Decode `bytes`, then resize + re-encode into the rendition set (each label in
/// each enabled format). Returns `None` when the bytes aren't a decodable raster
/// image (e.g. a format we didn't enable a decoder for, or a corrupt upload) —
/// callers fall back to keeping just the original file. The `full` rendition
/// carries the original dimensions. Pure and CPU-bound: run inside `spawn_blocking`.
pub fn generate_renditions(bytes: &[u8]) -> Option<Vec<GeneratedVariant>> {
    let img = image::load_from_memory(bytes).ok()?;
    let (ow, oh) = (img.width(), img.height());
    if ow == 0 || oh == 0 {
        return None;
    }

    // Width buckets strictly smaller than the original (never upscale), then a
    // full-size re-encode.
    let mut targets: Vec<(&'static str, u32)> = TARGET_WIDTHS
        .iter()
        .copied()
        .filter(|(_, w)| *w < ow)
        .collect();
    targets.push(("full", ow));

    let mut variants = Vec::new();
    for (label, w) in targets {
        let resized = if w >= ow {
            img.clone()
        } else {
            // Preserve aspect ratio: derive the height from the target width.
            let h = ((oh as u64 * w as u64) / ow as u64).max(1) as u32;
            img.resize_exact(w, h, image::imageops::FilterType::Lanczos3)
        };
        let rgba = resized.to_rgba8();
        let (rw, rh) = (rgba.width(), rgba.height());

        // WebP via libwebp (the pure-Rust `image` encoder is lossless-only, which
        // would defeat the point). `from_rgba` is infallible for 8-bit RGBA.
        let webp = webp::Encoder::from_rgba(rgba.as_raw(), rw, rh).encode(WEBP_QUALITY);
        variants.push(GeneratedVariant {
            label,
            format: "webp",
            width: rw,
            height: rh,
            bytes: webp.to_vec(),
        });

        #[cfg(feature = "avif")]
        if let Some(av) = encode_avif(&rgba, rw, rh) {
            variants.push(GeneratedVariant {
                label,
                format: "avif",
                width: rw,
                height: rh,
                bytes: av,
            });
        }
    }

    Some(variants)
}

/// Encode an RGBA buffer as AVIF using the pure-Rust encoder. Only compiled with
/// the `avif` feature. Speed 6 / quality 60 is a reasonable balance; bump quality
/// for sharper output at the cost of larger files and slower encodes.
#[cfg(feature = "avif")]
fn encode_avif(rgba: &image::RgbaImage, w: u32, h: u32) -> Option<Vec<u8>> {
    use image::codecs::avif::AvifEncoder;
    use image::{ExtendedColorType, ImageEncoder};

    let mut buf = Vec::new();
    AvifEncoder::new_with_speed_quality(&mut buf, 6, 60)
        .write_image(rgba.as_raw(), w, h, ExtendedColorType::Rgba8)
        .ok()?;
    Some(buf)
}

/// The shared `<img …>` matcher. Captures the whole tag (group 0) and its `src`
/// (group 1). Ammonia emits attributes in a normalized order, so `src` may sit
/// after other attributes — the lazy prefix handles that.
fn img_tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"<img\b[^>]*?\bsrc="([^"]+)"[^>]*>"#).unwrap())
}

/// Distinct `/uploads/…` image sources referenced by `<img>` tags in `html` —
/// the set whose renditions the caller looks up before rewriting.
pub fn collect_upload_img_srcs(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    for caps in img_tag_re().captures_iter(html) {
        let src = caps[1].to_string();
        if src.starts_with("/uploads/") && !out.contains(&src) {
            out.push(src);
        }
    }
    out
}

/// Rewrite every `<img src="/uploads/…">` that has renditions into a `<picture>`
/// with AVIF/WebP `<source>`s, keeping the original `<img>` as the fallback.
/// `srcsets` maps an image `src` to `(avif_srcset, webp_srcset)`. Tags without an
/// entry are left untouched.
///
/// The input is already-sanitized HTML and the injected markup is built only from
/// trusted DB values (our own `/uploads/…` rendition URLs) plus the original tag,
/// so the result is not re-sanitized.
pub fn rewrite_inline_images(
    html: &str,
    srcsets: &HashMap<String, (Option<String>, Option<String>)>,
    sizes: &str,
) -> String {
    if srcsets.is_empty() {
        return html.to_string();
    }
    img_tag_re()
        .replace_all(html, |caps: &regex::Captures| {
            let whole = &caps[0];
            let src = &caps[1];
            match srcsets.get(src) {
                Some((avif, webp)) if avif.is_some() || webp.is_some() => {
                    let mut sources = String::new();
                    if let Some(a) = avif {
                        sources.push_str(&format!(
                            r#"<source type="image/avif" srcset="{a}" sizes="{sizes}">"#
                        ));
                    }
                    if let Some(w) = webp {
                        sources.push_str(&format!(
                            r#"<source type="image/webp" srcset="{w}" sizes="{sizes}">"#
                        ));
                    }
                    format!("<picture>{sources}{whole}</picture>")
                }
                _ => whole.to_string(),
            }
        })
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode a solid `w×h` PNG in memory for the rendition tests.
    fn png(w: u32, h: u32) -> Vec<u8> {
        use image::{DynamicImage, ImageFormat, RgbImage};
        let img = RgbImage::from_fn(w, h, |x, _| image::Rgb([(x % 256) as u8, 100, 150]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn renditions_never_upscale_and_keep_aspect() {
        // 800×600: thumb(320)+small(640) fit under 800; medium(1024)/large(1600)
        // are dropped; a full re-encode is always added.
        let r = generate_renditions(&png(800, 600)).expect("decodes");
        assert!(
            r.iter().all(|v| v.width <= 800),
            "must never upscale beyond the original width"
        );

        let webp: Vec<_> = r
            .iter()
            .filter(|v| v.format == "webp")
            .map(|v| v.label)
            .collect();
        assert!(webp.contains(&"thumb"));
        assert!(webp.contains(&"small"));
        assert!(webp.contains(&"full"));
        assert!(!webp.contains(&"medium"), "1024 > 800, should be skipped");
        assert!(!webp.contains(&"large"));

        // The `full` rendition carries the original dimensions.
        let full = r
            .iter()
            .find(|v| v.label == "full" && v.format == "webp")
            .unwrap();
        assert_eq!((full.width, full.height), (800, 600));

        let thumb = r
            .iter()
            .find(|v| v.label == "thumb" && v.format == "webp")
            .unwrap();
        assert_eq!((thumb.width, thumb.height), (320, 240), "aspect preserved");
        assert!(!thumb.bytes.is_empty());
    }

    #[test]
    fn tiny_image_yields_only_full() {
        // 100×80 is smaller than every bucket → just a full re-encode.
        let r = generate_renditions(&png(100, 80)).expect("decodes");
        let labels: Vec<_> = r.iter().map(|v| v.label).collect();
        assert!(labels.iter().all(|l| *l == "full"));
    }

    #[test]
    fn undecodable_bytes_return_none() {
        assert!(generate_renditions(b"not an image").is_none());
    }

    #[test]
    fn rewrite_wraps_only_known_uploads() {
        let mut map = HashMap::new();
        map.insert(
            "/uploads/3_a.jpg".to_string(),
            (
                Some("/uploads/3_a-thumb.avif 320w".to_string()),
                Some("/uploads/3_a-thumb.webp 320w".to_string()),
            ),
        );
        let html = r#"<p><img alt="cat" src="/uploads/3_a.jpg"></p><img src="/uploads/none.jpg">"#;
        let out = rewrite_inline_images(html, &map, "100vw");

        assert_eq!(out.matches("<picture>").count(), 1, "only the mapped img");
        assert!(out.contains(r#"<source type="image/avif" srcset="/uploads/3_a-thumb.avif 320w""#));
        assert!(out.contains(r#"<source type="image/webp" srcset="/uploads/3_a-thumb.webp 320w""#));
        // Original tag (with its alt) is kept as the fallback inside <picture>.
        assert!(out.contains(r#"<img alt="cat" src="/uploads/3_a.jpg">"#));
        // The unmapped upload is left exactly as-is.
        assert!(out.contains(r#"<img src="/uploads/none.jpg">"#));
    }

    #[test]
    fn collect_dedups_and_ignores_external() {
        let html = concat!(
            r#"<img src="/uploads/1_a.png">"#,
            r#"<img src="https://cdn.example/x.png">"#,
            r#"<img alt="y" src="/uploads/1_a.png">"#,
        );
        assert_eq!(collect_upload_img_srcs(html), vec!["/uploads/1_a.png"]);
    }
}
