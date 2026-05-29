//! Authoring & admin mutations. Capability gating via global permission tokens;
//! per-post edit/delete via arium's resource-or-permission check (Editor on the
//! post OR the global `posts:write_any` admin token).

use dioxus::prelude::*;

use crate::model::{Category, CommentView, MediaItem, PostCard, PostEditData, Tag};

#[cfg(feature = "server")]
use crate::auth_tokens::{
    COMMENTS_MODERATE, MEDIA_UPLOAD, POSTS_WRITE, POSTS_WRITE_ANY, SETTINGS_WRITE,
};
#[cfg(feature = "server")]
use crate::server::{
    render_markdown, require_perm, sfe, unique_slug, DbExtension, POST_CARD_COLUMNS,
    POST_CARD_JOINS,
};

// ---------------------------------------------------------------- helper

/// Edit/delete authorization: Editor+ on the post, OR a global admin token.
#[cfg(feature = "server")]
pub(crate) async fn can_edit_post(
    auth: &arium_dioxus::auth::Session,
    db: &arium_dioxus::pool::Pool,
    authority: &arium_dioxus::ResourceAuthorityExt,
    post_id: i64,
) -> std::result::Result<i64, ServerFnError> {
    let uid = auth
        .current_user
        .as_ref()
        .filter(|u| !u.anonymous)
        .map(|u| u.id as i64)
        .ok_or_else(|| ServerFnError::new("Not signed in."))?;
    arium_dioxus::require_resource_or_permission(
        authority.0.as_ref(),
        db,
        uid,
        arium_dioxus::ResourceRef::new("post", post_id),
        arium_dioxus::ResourceRole::Editor,
        POSTS_WRITE_ANY,
    )
    .await
    .map_err(|_| ServerFnError::new("You can't edit this post."))?;
    Ok(uid)
}

// ---------------------------------------------------------------- posts

/// Create a post. Requires the `posts:write` capability; the creator becomes
/// the post's resource Owner.
#[post("/api/posts/create", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn create_post(
    title: String,
    body_md: String,
    excerpt: String,
    category_id: Option<i64>,
    tag_ids: Vec<i64>,
    featured_image_url: Option<String>,
    status: String,
) -> Result<i64> {
    let uid = require_perm(&auth, POSTS_WRITE)?;
    let slug = unique_slug(&db.0, "posts", &title).await.map_err(sfe)?;
    let body_html = render_markdown(&body_md);

    let post_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO posts
          (title, slug, body_md, body_html, excerpt, author_id, category_id,
           featured_image_url, status, published_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?,
           CASE WHEN ? = 'published' THEN datetime('now') ELSE NULL END)
        RETURNING id
        "#,
    )
    .bind(&title)
    .bind(&slug)
    .bind(&body_md)
    .bind(&body_html)
    .bind(&excerpt)
    .bind(uid)
    .bind(category_id)
    .bind(&featured_image_url)
    .bind(&status)
    .bind(&status)
    .fetch_one(&db.0)
    .await
    .map_err(sfe)?;

    set_post_tags(&db.0, post_id, &tag_ids).await?;

    // Creator becomes Owner. Direct upsert bootstraps the membership (grant_membership
    // requires a pre-existing Manager, which a brand-new post has none of).
    sqlx::query(
        "INSERT INTO arium_resource_members (kind, resource_id, user_id, role)
         VALUES ('post', ?, ?, ?)
         ON CONFLICT (kind, resource_id, user_id) DO UPDATE SET role = excluded.role",
    )
    .bind(post_id)
    .bind(uid)
    .bind(arium_dioxus::ResourceRole::Owner.as_str())
    .execute(&db.0)
    .await
    .map_err(sfe)?;

    Ok(post_id)
}

/// Update an existing post (Editor on the post, or admin token).
#[post("/api/posts/update", auth: arium_dioxus::auth::Session, db: DbExtension, authority: arium_dioxus::ResourceAuthorityExt)]
pub async fn update_post(
    id: i64,
    title: String,
    body_md: String,
    excerpt: String,
    category_id: Option<i64>,
    tag_ids: Vec<i64>,
    featured_image_url: Option<String>,
    status: String,
) -> Result<()> {
    can_edit_post(&auth, &db.0, &authority, id).await?;
    let body_html = render_markdown(&body_md);

    sqlx::query(
        r#"
        UPDATE posts SET
          title = ?, body_md = ?, body_html = ?, excerpt = ?,
          category_id = ?, featured_image_url = ?, status = ?,
          published_at = CASE WHEN ? = 'published' AND published_at IS NULL
                              THEN datetime('now') ELSE published_at END,
          updated_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(&title)
    .bind(&body_md)
    .bind(&body_html)
    .bind(&excerpt)
    .bind(category_id)
    .bind(&featured_image_url)
    .bind(&status)
    .bind(&status)
    .bind(id)
    .execute(&db.0)
    .await
    .map_err(sfe)?;

    set_post_tags(&db.0, id, &tag_ids).await?;
    Ok(())
}

/// Delete a post (admin only).
#[post("/api/posts/delete", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn delete_post(id: i64) -> Result<()> {
    require_perm(&auth, POSTS_WRITE_ANY)?;
    sqlx::query("DELETE FROM post_tags WHERE post_id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    sqlx::query("DELETE FROM comments WHERE post_id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    // Recorded views for this post — otherwise they pile up unbounded.
    sqlx::query("DELETE FROM post_views WHERE post_id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    // Per-post ownership/membership rows (kind='post'); otherwise stale grants linger.
    sqlx::query("DELETE FROM arium_resource_members WHERE kind = 'post' AND resource_id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    sqlx::query("DELETE FROM posts WHERE id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}

/// Posts visible to the current author/admin (admins see all; authors see own),
/// optionally filtered by status and sorted. `status_filter` is `None`/empty for
/// all statuses; `sort` is one of a fixed whitelist (the ORDER BY clause is
/// chosen from constants, never interpolated from user input).
#[post("/api/admin/posts", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn admin_list_posts(
    status_filter: Option<String>,
    sort: Option<String>,
) -> Result<Vec<PostCard>> {
    let uid = require_perm(&auth, POSTS_WRITE)?;
    let is_admin = auth
        .current_user
        .as_ref()
        .map(|u| u.permissions.contains(POSTS_WRITE_ANY))
        .unwrap_or(false);

    let status_filter = status_filter.filter(|s| !s.is_empty());

    let order_by = match sort.as_deref() {
        Some("title") => "p.title COLLATE NOCASE ASC, p.id DESC",
        Some("title_desc") => "p.title COLLATE NOCASE DESC, p.id DESC",
        Some("status") => "p.status ASC, p.updated_at DESC",
        Some("published") => "p.published_at IS NULL, p.published_at DESC, p.id DESC",
        Some("oldest") => "p.updated_at ASC, p.id ASC",
        // "recent" / None / anything unrecognised
        _ => "p.updated_at DESC, p.id DESC",
    };

    let sql = format!(
        "SELECT {POST_CARD_COLUMNS} FROM posts p {POST_CARD_JOINS} \
         WHERE (? = 1 OR p.author_id = ?) AND (? IS NULL OR p.status = ?) \
         ORDER BY {order_by}"
    );

    let rows = sqlx::query_as::<_, PostCard>(&sql)
        .bind(is_admin as i64)
        .bind(uid)
        .bind(&status_filter)
        .bind(&status_filter)
        .fetch_all(&db.0)
        .await
        .map_err(sfe)?;
    Ok(rows)
}

/// Raw fields for the edit form (Editor on the post, or admin token).
#[post("/api/admin/post-edit", auth: arium_dioxus::auth::Session, db: DbExtension, authority: arium_dioxus::ResourceAuthorityExt)]
pub async fn get_post_edit(id: i64) -> Result<PostEditData> {
    can_edit_post(&auth, &db.0, &authority, id).await?;

    let row: (
        String,
        String,
        String,
        String,
        Option<i64>,
        Option<String>,
        String,
    ) = sqlx::query_as(
        "SELECT title, slug, body_md, excerpt, category_id, featured_image_url, status
             FROM posts WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&db.0)
    .await
    .map_err(sfe)?;

    let tag_ids: Vec<i64> = sqlx::query_scalar("SELECT tag_id FROM post_tags WHERE post_id = ?")
        .bind(id)
        .fetch_all(&db.0)
        .await
        .map_err(sfe)?;

    Ok(PostEditData {
        id,
        title: row.0,
        slug: row.1,
        body_md: row.2,
        excerpt: row.3,
        category_id: row.4,
        featured_image_url: row.5,
        status: row.6,
        tag_ids,
    })
}

#[cfg(feature = "server")]
async fn set_post_tags(
    pool: &arium_dioxus::pool::Pool,
    post_id: i64,
    tag_ids: &[i64],
) -> std::result::Result<(), ServerFnError> {
    sqlx::query("DELETE FROM post_tags WHERE post_id = ?")
        .bind(post_id)
        .execute(pool)
        .await
        .map_err(sfe)?;
    for tid in tag_ids {
        sqlx::query("INSERT OR IGNORE INTO post_tags (post_id, tag_id) VALUES (?, ?)")
            .bind(post_id)
            .bind(tid)
            .execute(pool)
            .await
            .map_err(sfe)?;
    }
    Ok(())
}

/// Render Markdown to sanitized HTML for the editor's live preview.
#[post("/api/admin/preview", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn preview_markdown(md: String) -> Result<String> {
    require_perm(&auth, POSTS_WRITE)?;
    let _ = &db; // pool unused; extractor kept for a uniform signature
    Ok(render_markdown(&md))
}

// ---------------------------------------------------------------- comments

/// Comments filtered by status (defaults to all) — moderation queue.
#[post("/api/admin/comments", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn admin_list_comments(status: Option<String>) -> Result<Vec<CommentView>> {
    require_perm(&auth, COMMENTS_MODERATE)?;
    let rows = sqlx::query_as::<_, CommentView>(
        r#"
        SELECT cm.id, cm.post_id,
               COALESCE(u.display_name, u.username, cm.guest_name, 'Anonymous') AS display_name,
               cm.body, cm.status, cm.created_at
        FROM comments cm
        LEFT JOIN users u ON u.id = cm.author_id
        WHERE (? IS NULL OR cm.status = ?)
        ORDER BY cm.created_at DESC
        "#,
    )
    .bind(&status)
    .bind(&status)
    .fetch_all(&db.0)
    .await
    .map_err(sfe)?;
    Ok(rows)
}

#[post("/api/admin/comments/moderate", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn moderate_comment(id: i64, status: String) -> Result<()> {
    require_perm(&auth, COMMENTS_MODERATE)?;
    if !["pending", "approved", "rejected"].contains(&status.as_str()) {
        return Err(ServerFnError::new("Invalid status.").into());
    }
    sqlx::query("UPDATE comments SET status = ? WHERE id = ?")
        .bind(&status)
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}

#[post("/api/admin/comments/delete", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn delete_comment(id: i64) -> Result<()> {
    require_perm(&auth, COMMENTS_MODERATE)?;
    sqlx::query("DELETE FROM comments WHERE id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}

// ---------------------------------------------------------------- taxonomy CRUD

#[post("/api/admin/categories/create", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn create_category(name: String, description: Option<String>) -> Result<Category> {
    require_perm(&auth, SETTINGS_WRITE)?;
    let slug = unique_slug(&db.0, "categories", &name).await.map_err(sfe)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO categories (name, slug, description) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(&name)
    .bind(&slug)
    .bind(&description)
    .fetch_one(&db.0)
    .await
    .map_err(sfe)?;
    Ok(Category {
        id,
        name,
        slug,
        description,
    })
}

#[post("/api/admin/categories/delete", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn delete_category(id: i64) -> Result<()> {
    require_perm(&auth, SETTINGS_WRITE)?;
    sqlx::query("DELETE FROM categories WHERE id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}

/// Rename a category. The slug is kept stable so existing `/category/:slug`
/// links keep resolving; only the display name changes.
#[post("/api/admin/categories/rename", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn rename_category(id: i64, name: String) -> Result<()> {
    require_perm(&auth, SETTINGS_WRITE)?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ServerFnError::new("Name can't be empty.").into());
    }
    sqlx::query("UPDATE categories SET name = ? WHERE id = ?")
        .bind(&name)
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}

#[post("/api/admin/tags/create", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn create_tag(name: String) -> Result<Tag> {
    require_perm(&auth, SETTINGS_WRITE)?;
    let slug = unique_slug(&db.0, "tags", &name).await.map_err(sfe)?;
    let id: i64 = sqlx::query_scalar("INSERT INTO tags (name, slug) VALUES (?, ?) RETURNING id")
        .bind(&name)
        .bind(&slug)
        .fetch_one(&db.0)
        .await
        .map_err(sfe)?;
    Ok(Tag { id, name, slug })
}

#[post("/api/admin/tags/delete", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn delete_tag(id: i64) -> Result<()> {
    require_perm(&auth, SETTINGS_WRITE)?;
    sqlx::query("DELETE FROM post_tags WHERE tag_id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    sqlx::query("DELETE FROM tags WHERE id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}

/// Rename a tag. As with categories, the slug stays put so existing
/// `/tag/:slug` links keep working.
#[post("/api/admin/tags/rename", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn rename_tag(id: i64, name: String) -> Result<()> {
    require_perm(&auth, SETTINGS_WRITE)?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ServerFnError::new("Name can't be empty.").into());
    }
    sqlx::query("UPDATE tags SET name = ? WHERE id = ?")
        .bind(&name)
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;
    Ok(())
}

// ---------------------------------------------------------------- media

#[get("/api/admin/media", auth: arium_dioxus::auth::Session, db: DbExtension)]
pub async fn list_media() -> Result<Vec<MediaItem>> {
    require_perm(&auth, MEDIA_UPLOAD)?;
    let rows = sqlx::query_as::<_, MediaItem>(
        "SELECT id, filename, url, uploaded_by, created_at FROM media ORDER BY created_at DESC",
    )
    .fetch_all(&db.0)
    .await
    .map_err(sfe)?;
    Ok(rows)
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

    // Reserve a row to get a unique id, then write the file and fill in the url.
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO media (filename, url, uploaded_by) VALUES (?, '', ?) RETURNING id",
    )
    .bind(&safe)
    .bind(uid)
    .fetch_one(&db.0)
    .await
    .map_err(sfe)?;

    let stored = format!("{id}_{safe}");
    let url = format!("/uploads/{stored}");
    std::fs::create_dir_all("uploads").map_err(sfe)?;
    std::fs::write(format!("uploads/{stored}"), &bytes).map_err(sfe)?;

    sqlx::query("UPDATE media SET url = ? WHERE id = ?")
        .bind(&url)
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;

    let created_at: String = sqlx::query_scalar("SELECT created_at FROM media WHERE id = ?")
        .bind(id)
        .fetch_one(&db.0)
        .await
        .map_err(sfe)?;

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
    require_perm(&auth, MEDIA_UPLOAD)?;

    // Grab the stored url first so we can unlink the file after dropping the row.
    let url: Option<String> = sqlx::query_scalar("SELECT url FROM media WHERE id = ?")
        .bind(id)
        .fetch_optional(&db.0)
        .await
        .map_err(sfe)?;

    sqlx::query("DELETE FROM media WHERE id = ?")
        .bind(id)
        .execute(&db.0)
        .await
        .map_err(sfe)?;

    // Best-effort: remove the file on disk so uploads don't accumulate forever.
    // The url is `/uploads/<id>_<filename>`; map it back to the on-disk path. A
    // failure here (already gone, permissions) must not fail the request.
    if let Some(name) = url.as_deref().and_then(|u| u.strip_prefix("/uploads/")) {
        let _ = std::fs::remove_file(format!("uploads/{name}"));
    }
    Ok(())
}
