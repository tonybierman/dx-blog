//! Post read endpoints (public). Authoring/mutation lives in `server::admin`.

use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::model::{page_offset, PER_PAGE};
use crate::model::{PostDetail, PostFeed};

#[cfg(feature = "server")]
use crate::db::posts::{
    featured_posts_db, get_post_db, list_archive_db, list_posts_db, posts_by_author_db,
};
#[cfg(feature = "server")]
use crate::server::{sfe, DbExtension};

/// Paginated published-post feed, optionally filtered by category or tag slug.
#[post("/api/posts", db: DbExtension)]
pub async fn list_posts(
    page: i64,
    category_slug: Option<String>,
    tag_slug: Option<String>,
) -> Result<PostFeed> {
    let (page, offset) = page_offset(page);
    let (items, total) = list_posts_db(
        &db.0,
        PER_PAGE,
        offset,
        category_slug.as_deref(),
        tag_slug.as_deref(),
    )
    .await
    .map_err(sfe)?;
    Ok(PostFeed::new(items, total, page))
}

/// All published posts, newest first — backs the Masonry archive.
#[post("/api/archive", db: DbExtension)]
pub async fn list_archive(page: i64) -> Result<PostFeed> {
    let (page, offset) = page_offset(page);
    let (items, total) = list_archive_db(&db.0, PER_PAGE, offset)
        .await
        .map_err(sfe)?;
    Ok(PostFeed::new(items, total, page))
}

/// Published posts authored by a given username, paginated.
#[post("/api/author-posts", db: DbExtension)]
pub async fn posts_by_author(username: String, page: i64) -> Result<PostFeed> {
    let (page, offset) = page_offset(page);
    let (items, total) = posts_by_author_db(&db.0, &username, PER_PAGE, offset)
        .await
        .map_err(sfe)?;
    Ok(PostFeed::new(items, total, page))
}

/// The most-viewed published posts — backs the home "Featured" sidebar.
#[post("/api/posts/featured", db: DbExtension)]
pub async fn featured_posts(limit: i64) -> Result<Vec<crate::model::PostCard>> {
    let limit = limit.clamp(1, 10);
    Ok(featured_posts_db(&db.0, limit).await.map_err(sfe)?)
}

/// A single post by slug. Published posts are public; drafts are only returned
/// to callers who can edit them (Editor+ on the post, or a global admin).
#[post("/api/post", auth: arium_dioxus::auth::Session, db: DbExtension, authority: arium_dioxus::ResourceAuthorityExt)]
pub async fn get_post(slug: String) -> Result<Option<PostDetail>> {
    let post = get_post_db(&db.0, &slug).await.map_err(sfe)?;
    if let Some(ref p) = post {
        if p.status != "published"
            && crate::server::admin::can_edit_post(&auth, &db.0, &authority, p.id)
                .await
                .is_err()
        {
            return Ok(None);
        }
    }
    Ok(post)
}
