//! Wire types shared across the client/server boundary. `serde` derives are
//! always present; `sqlx::FromRow` is added only on the server build so the
//! same struct can be selected straight out of SQLite.

use serde::{Deserialize, Serialize};

macro_rules! wire_struct {
    ($(#[$m:meta])* pub struct $name:ident { $($(#[$fm:meta])* pub $field:ident : $ty:ty),* $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Default)]
        #[cfg_attr(feature = "server", derive(sqlx::FromRow))]
        pub struct $name {
            $($(#[$fm])* pub $field : $ty,)*
        }
    };
}

wire_struct! {
    /// Summary card for feeds/listings.
    pub struct PostCard {
        pub id: i64,
        pub title: String,
        pub slug: String,
        pub excerpt: String,
        pub featured_image_url: Option<String>,
        pub author_id: i64,
        pub author_name: String,
        pub category_name: Option<String>,
        pub status: String,
        pub published_at: Option<String>,
    }
}

wire_struct! {
    /// Full article for the detail page.
    pub struct PostDetail {
        pub id: i64,
        pub title: String,
        pub slug: String,
        pub body_md: String,
        pub body_html: String,
        pub excerpt: String,
        pub featured_image_url: Option<String>,
        pub author_id: i64,
        pub author_name: String,
        pub author_username: String,
        pub author_bio: Option<String>,
        pub category_id: Option<i64>,
        pub category_name: Option<String>,
        pub status: String,
        pub published_at: Option<String>,
        pub created_at: String,
    }
}

wire_struct! {
    pub struct Category {
        pub id: i64,
        pub name: String,
        pub slug: String,
        pub description: Option<String>,
    }
}

wire_struct! {
    pub struct Tag {
        pub id: i64,
        pub name: String,
        pub slug: String,
    }
}

wire_struct! {
    /// A comment as shown publicly (display name resolved server-side).
    pub struct CommentView {
        pub id: i64,
        pub post_id: i64,
        pub display_name: String,
        pub body: String,
        pub status: String,
        pub created_at: String,
    }
}

wire_struct! {
    pub struct AuthorProfile {
        pub user_id: i64,
        pub username: String,
        pub display_name: String,
        pub avatar_url: Option<String>,
        pub bio: Option<String>,
        pub social_links: Option<String>,
    }
}

wire_struct! {
    pub struct MediaItem {
        pub id: i64,
        pub filename: String,
        pub url: String,
        pub uploaded_by: i64,
        pub created_at: String,
    }
}

wire_struct! {
    /// Aggregate counts for the admin dashboard / analytics.
    pub struct AnalyticsSummary {
        pub post_count: i64,
        pub published_count: i64,
        pub draft_count: i64,
        pub comment_count: i64,
        pub pending_comment_count: i64,
        pub subscriber_count: i64,
        pub view_count: i64,
    }
}

/// Raw post fields for the editor (assembled server-side; not a single row
/// because `tag_ids` comes from a join table).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct PostEditData {
    pub id: i64,
    pub title: String,
    pub slug: String,
    pub body_md: String,
    pub excerpt: String,
    pub category_id: Option<i64>,
    pub featured_image_url: Option<String>,
    pub status: String,
    pub tag_ids: Vec<i64>,
}

/// A page of post cards plus the total count, for offset pagination.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct PostFeed {
    pub items: Vec<PostCard>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

impl PostFeed {
    pub fn total_pages(&self) -> i64 {
        if self.per_page <= 0 {
            return 1;
        }
        ((self.total + self.per_page - 1) / self.per_page).max(1)
    }
}

/// Default page size for listings.
pub const PER_PAGE: i64 = 10;
