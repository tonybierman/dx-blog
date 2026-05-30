//! Media library: list, upload (base64 → ./uploads, served at /uploads), delete.

use dioxus::prelude::*;

use crate::model::{MediaItem, PostUsage};

#[cfg(feature = "server")]
use crate::auth_tokens::{MEDIA_UPLOAD, POSTS_WRITE_ANY};
#[cfg(feature = "server")]
use crate::db::media::{
    delete_media_db, get_media_created_at_db, get_media_row_db, insert_media_stub_db,
    insert_variant_db, list_media_db, media_usage_db, post_image_corpus_db, update_media_url_db,
    variant_urls_for_media_db,
};
#[cfg(feature = "server")]
use crate::server::{require_perm, sfe, DbExtension};

#[get("/api/admin/media", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn list_media() -> Result<Vec<MediaItem>> {
    require_perm(&auth, MEDIA_UPLOAD)?;
    let mut items = list_media_db(&db.0).await.map_err(sfe)?;

    // Derive each item's usage count in one pass over the post corpus (cover =
    // featured URL match; body = the URL embedded in the markdown), rather than a
    // query per media row. Accurate by construction — usage isn't stored anywhere
    // to fall out of sync.
    let corpus = post_image_corpus_db(&db.0).await.map_err(sfe)?;
    for m in &mut items {
        m.usage_count = corpus
            .iter()
            .filter(|(featured, body)| {
                featured.as_deref() == Some(m.url.as_str()) || body.contains(&m.url)
            })
            .count() as i64;
    }
    Ok(items)
}

/// Which posts use a given media item (cover and/or inline body). Backs the media
/// library's usage panel and the delete confirmation.
#[post("/api/admin/media/usage", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn media_usage(id: i64) -> Result<Vec<PostUsage>> {
    require_perm(&auth, MEDIA_UPLOAD)?;
    let Some((url, _)) = get_media_row_db(&db.0, id).await.map_err(sfe)? else {
        return Ok(Vec::new());
    };
    Ok(media_usage_db(&db.0, &url).await.map_err(sfe)?)
}

/// Upload an image (base64-encoded). Stored under ./uploads and served at /uploads.
#[post("/api/admin/media/upload", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn upload_media(filename: String, data_base64: String) -> Result<MediaItem> {
    use base64::{engine::general_purpose::STANDARD, Engine};

    let uid = require_perm(&auth, MEDIA_UPLOAD)?;

    // Allow-list raster image extensions only. Uploads are served from the app
    // origin by ServeDir, which derives Content-Type from the extension — so an
    // `.html` or scripted `.svg` would be served as live, same-origin content and
    // execute JS on the site. `accept="image/*"` in the form is a client-side hint
    // only; this server-side check is the real gate. SVG is intentionally excluded
    // (it can carry <script>).
    const ALLOWED_EXT: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "avif"];
    let ext_ok = std::path::Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| ALLOWED_EXT.contains(&e.as_str()));
    if !ext_ok {
        return Err(ServerFnError::new(
            "Unsupported file type. Allowed: png, jpg, jpeg, gif, webp, avif.",
        )
        .into());
    }

    let bytes = STANDARD
        .decode(data_base64.as_bytes())
        .map_err(|_| ServerFnError::new("Invalid file data."))?;

    // Cap the decoded size (10 MiB) so a holder of the upload token can't fill the
    // disk with one request.
    const MAX_BYTES: usize = 10 * 1024 * 1024;
    if bytes.len() > MAX_BYTES {
        return Err(ServerFnError::new("File too large (max 10 MB).").into());
    }

    let safe: String = filename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();

    let id = insert_media_stub_db(&db.0, &safe, uid).await.map_err(sfe)?;

    let stored = format!("{id}_{safe}");
    let url = format!("/uploads/{stored}");
    std::fs::create_dir_all("uploads").map_err(sfe)?;
    std::fs::write(format!("uploads/{stored}"), &bytes).map_err(sfe)?;

    update_media_url_db(&db.0, id, &url).await.map_err(sfe)?;

    // Generate WordPress-style responsive renditions (WebP, plus AVIF when built
    // with the `avif` feature) off the decoded bytes. Best-effort: a decode/encode
    // failure (e.g. an animated GIF or a format we don't decode) just leaves the
    // original standing with no `srcset`. The CPU-bound work runs on a blocking
    // thread so it doesn't stall the async runtime.
    let stem = stored
        .rsplit_once('.')
        .map(|(s, _)| s.to_string())
        .unwrap_or_else(|| stored.clone());
    let data = bytes.clone();
    match tokio::task::spawn_blocking(move || crate::server::images::generate_renditions(&data))
        .await
    {
        Ok(Some(rends)) => {
            for v in rends {
                let fname = format!("{stem}-{}.{}", v.label, v.format);
                if std::fs::write(format!("uploads/{fname}"), &v.bytes).is_ok() {
                    let vurl = format!("/uploads/{fname}");
                    let _ = insert_variant_db(
                        &db.0,
                        id,
                        v.label,
                        v.format,
                        v.width as i64,
                        v.height as i64,
                        &vurl,
                        v.bytes.len() as i64,
                    )
                    .await;
                }
            }
        }
        Ok(None) => {
            tracing::info!(target: "media", "media {id}: no renditions (undecodable image?)");
        }
        Err(e) => {
            tracing::warn!(target: "media", "media {id}: rendition task failed: {e}");
        }
    }

    let created_at = get_media_created_at_db(&db.0, id).await.map_err(sfe)?;

    Ok(MediaItem {
        id,
        filename: safe,
        url,
        uploaded_by: uid,
        created_at,
        usage_count: 0,
    })
}

#[post("/api/admin/media/delete", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn delete_media(id: i64) -> Result<()> {
    let uid = require_perm(&auth, MEDIA_UPLOAD)?;

    // Grab the stored url + owner first: the url so we can unlink the file after
    // dropping the row, the owner to enforce that an author may only delete their
    // own uploads. The MEDIA_UPLOAD token alone would otherwise let any author
    // delete anyone's media (IDOR within the role); a global admin overrides.
    let row = get_media_row_db(&db.0, id).await.map_err(sfe)?;
    let Some((url, uploaded_by)) = row else {
        return Ok(()); // already gone — nothing to do
    };
    let is_admin = auth
        .current_user
        .as_ref()
        .map(|u| u.permissions.contains(POSTS_WRITE_ANY))
        .unwrap_or(false);
    if uploaded_by != uid && !is_admin {
        return Err(ServerFnError::new("You can only delete media you uploaded.").into());
    }

    // Grab the rendition file paths before the rows cascade away with the parent.
    let variant_urls = variant_urls_for_media_db(&db.0, id)
        .await
        .unwrap_or_default();

    delete_media_db(&db.0, id).await.map_err(sfe)?;

    // Best-effort: remove the original and every rendition on disk so uploads
    // don't accumulate forever.
    for u in std::iter::once(url).chain(variant_urls) {
        if let Some(name) = u.strip_prefix("/uploads/") {
            let _ = std::fs::remove_file(format!("uploads/{name}"));
        }
    }
    Ok(())
}
