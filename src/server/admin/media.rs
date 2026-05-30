//! Media library: list, upload (base64 → ./uploads, served at /uploads), delete.

use dioxus::prelude::*;

use crate::model::MediaItem;

#[cfg(feature = "server")]
use crate::auth_tokens::{MEDIA_UPLOAD, POSTS_WRITE_ANY};
#[cfg(feature = "server")]
use crate::db::media::{
    delete_media_db, get_media_created_at_db, get_media_row_db, insert_media_stub_db,
    list_media_db, update_media_url_db,
};
#[cfg(feature = "server")]
use crate::server::{require_perm, sfe, DbExtension};

#[get("/api/admin/media", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn list_media() -> Result<Vec<MediaItem>> {
    require_perm(&auth, MEDIA_UPLOAD)?;
    Ok(list_media_db(&db.0).await.map_err(sfe)?)
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

    let created_at = get_media_created_at_db(&db.0, id).await.map_err(sfe)?;

    Ok(MediaItem {
        id,
        filename: safe,
        url,
        uploaded_by: uid,
        created_at,
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

    delete_media_db(&db.0, id).await.map_err(sfe)?;

    // Best-effort: remove the file on disk so uploads don't accumulate forever.
    if let Some(name) = url.strip_prefix("/uploads/") {
        let _ = std::fs::remove_file(format!("uploads/{name}"));
    }
    Ok(())
}
