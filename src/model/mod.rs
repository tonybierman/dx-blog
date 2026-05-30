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
    /// Site-level values used to build `<head>` / Open Graph tags: the display
    /// title, the tagline (used as the default description), and the canonical
    /// origin for turning relative paths into the absolute URLs OG requires.
    pub struct SiteMeta {
        pub title: String,
        pub tagline: String,
        pub base_url: String,
    }
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
        /// The body pre-rendered into highlighted HTML runs + embed blocks,
        /// filled server-side in `get_post` so the reader displays it without
        /// re-running the markdown/highlight pipeline on the client. Not a SQL
        /// column (`sqlx(skip)` → `Default`) and empty in every other context.
        #[cfg_attr(feature = "server", sqlx(skip))]
        #[serde(default)]
        pub body_segments: Vec<crate::mdx::Segment>,
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
    /// A recent approved comment with its post's title/slug, for the home sidebar.
    pub struct RecentComment {
        pub id: i64,
        pub post_title: String,
        pub post_slug: String,
        pub display_name: String,
        pub body: String,
        pub created_at: String,
    }
}

/// One message on a post's live (SSE) channel. The server serializes these onto
/// the stream; the wasm client (`crate::live`) parses them back. Not a
/// `wire_struct!` — it never touches sqlx, and the tagged enum is the natural
/// shape. Each variant is also sent under a matching SSE *event name*
/// (`presence`/`comment`/`reaction`); the `#[serde(tag = "type")]` envelope makes
/// the JSON self-describing so the client has a single parse path.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LiveEvent {
    /// Distinct readers currently connected to this post.
    Presence { count: i64 },
    /// A newly-visible (approved) comment, to append without a refetch.
    Comment(CommentView),
    /// A clap/reaction to animate in, plus the post's authoritative running
    /// total after it (so every client converges on the same count rather than
    /// each tracking its own increments).
    Reaction { kind: String, total: i64 },
    /// One sample on a named live data series, for charts embedded in the post
    /// body (the `livechart` embed). `topic` lets several charts on one page
    /// each follow their own feed; `value` is the newest point to append. Sent
    /// under the SSE event name `data`.
    Data { topic: String, value: f64 },
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

wire_struct! {
    /// One referrer source with its visit count, for the analytics page.
    pub struct ReferrerStat {
        pub referrer: String,
        pub views: i64,
    }
}

wire_struct! {
    /// Views recorded on a single day (`YYYY-MM-DD`), for the time-series chart.
    pub struct DailyViews {
        pub day: String,
        pub views: i64,
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
    /// Build a feed page with the standard [`PER_PAGE`] window, so the four
    /// paginated read paths (`list_posts` / `list_archive` / `posts_by_author` /
    /// `search_posts`) don't each repeat the struct literal + `per_page`.
    pub fn new(items: Vec<PostCard>, total: i64, page: i64) -> Self {
        PostFeed {
            items,
            total,
            page,
            per_page: PER_PAGE,
        }
    }

    /// An empty page (e.g. a blank search query). Page defaults to 1.
    pub fn empty() -> Self {
        PostFeed::new(Vec::new(), 0, 1)
    }

    pub fn total_pages(&self) -> i64 {
        if self.per_page <= 0 {
            return 1;
        }
        ((self.total + self.per_page - 1) / self.per_page).max(1)
    }
}

/// Default page size for listings.
pub const PER_PAGE: i64 = 10;

/// Clamp a 1-based page number to ≥ 1 and return `(page, offset)` for a
/// [`PER_PAGE`]-sized window. Shared by every paginated query.
pub fn page_offset(page: i64) -> (i64, i64) {
    let page = page.max(1);
    (page, (page - 1) * PER_PAGE)
}

/// Normalize a stored SQLite datetime (`YYYY-MM-DD HH:MM:SS`, UTC) to the ISO
/// 8601 / RFC 3339 form (`YYYY-MM-DDTHH:MM:SSZ`) that Atom, the sitemap, and Open
/// Graph all expect: just a separator swap plus a `Z` UTC marker. Empty input and
/// values already carrying a `T` pass through unchanged. Shared by the feed /
/// sitemap (server) and the per-post `<head>` tags (client).
pub fn to_rfc3339(dt: &str) -> String {
    let t = dt.trim();
    if t.is_empty() || t.contains('T') {
        return t.to_string();
    }
    format!("{}Z", t.replacen(' ', "T", 1))
}

/// The post lifecycle statuses. Single source of truth for the server-side
/// whitelist (mirrors the DB `CHECK`), the admin status `<select>`, and the
/// status filter — so the set lives in exactly one place.
pub const POST_STATUSES: [&str; 3] = ["draft", "published", "archived"];
pub const STATUS_DRAFT: &str = "draft";
pub const STATUS_PUBLISHED: &str = "published";

/// The comment moderation statuses (mirrors the DB `CHECK`).
pub const COMMENT_STATUSES: [&str; 3] = ["pending", "approved", "rejected"];

/// Which structural/marketing layout the public home page renders the post feed
/// in. Chosen by an admin in Settings and persisted in `site_settings`. The 12
/// variants are the dioxus-mcp registry's structural + marketing layout kinds
/// (excluding `admin_console`, which is admin chrome). The wire form is the
/// stable snake_case key (`as_key`); never serialize the enum's Rust name.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum HomeLayout {
    /// Header/footer with optional left + right sidebars flanking the feed.
    #[default]
    HolyGrail,
    /// A pinned side nav beside a scrolling feed.
    StickySidebar,
    /// Two equal panes: an inverted intro pane and the feed.
    SplitScreen,
    /// Edge-to-edge feed with no persistent chrome.
    FullBleed,
    /// Off-canvas nav panel toggled over a scrim above the feed.
    Drawer,
    /// Top bar with a drop-down mega panel above the feed.
    MegaMenu,
    /// Asymmetric tile grid of posts.
    BentoGrid,
    /// Staggered multi-column (CSS columns) feed.
    Masonry,
    /// Uniform responsive card grid.
    CardGrid,
    /// Centered reading measure with an asymmetric aside.
    Editorial,
    /// Full-viewport hero above the fold, feed below.
    HeroScroll,
    /// Scrolling posts beside a panel that pins in place.
    ScrollSticky,
}

impl HomeLayout {
    /// Every variant, in admin-display order. Source of truth for the selector.
    pub const ALL: [HomeLayout; 12] = [
        HomeLayout::HolyGrail,
        HomeLayout::StickySidebar,
        HomeLayout::SplitScreen,
        HomeLayout::FullBleed,
        HomeLayout::Drawer,
        HomeLayout::MegaMenu,
        HomeLayout::BentoGrid,
        HomeLayout::Masonry,
        HomeLayout::CardGrid,
        HomeLayout::Editorial,
        HomeLayout::HeroScroll,
        HomeLayout::ScrollSticky,
    ];

    /// Stable snake_case key for DB storage and the registry kind name.
    pub fn as_key(&self) -> &'static str {
        match self {
            HomeLayout::HolyGrail => "holy_grail",
            HomeLayout::StickySidebar => "sticky_sidebar",
            HomeLayout::SplitScreen => "split_screen",
            HomeLayout::FullBleed => "full_bleed",
            HomeLayout::Drawer => "drawer",
            HomeLayout::MegaMenu => "mega_menu",
            HomeLayout::BentoGrid => "bento_grid",
            HomeLayout::Masonry => "masonry",
            HomeLayout::CardGrid => "card_grid",
            HomeLayout::Editorial => "editorial",
            HomeLayout::HeroScroll => "hero_scroll",
            HomeLayout::ScrollSticky => "scroll_sticky",
        }
    }

    /// Parse a stored key back into a layout; `None` for unknown keys.
    pub fn from_key(key: &str) -> Option<HomeLayout> {
        HomeLayout::ALL.into_iter().find(|l| l.as_key() == key)
    }

    /// Human label for the admin selector.
    pub fn label(&self) -> &'static str {
        match self {
            HomeLayout::HolyGrail => "Holy Grail",
            HomeLayout::StickySidebar => "Sticky Sidebar",
            HomeLayout::SplitScreen => "Split Screen",
            HomeLayout::FullBleed => "Full-bleed",
            HomeLayout::Drawer => "Drawer",
            HomeLayout::MegaMenu => "Mega Menu",
            HomeLayout::BentoGrid => "Bento Grid",
            HomeLayout::Masonry => "Masonry",
            HomeLayout::CardGrid => "Card Grid",
            HomeLayout::Editorial => "Editorial",
            HomeLayout::HeroScroll => "Hero",
            HomeLayout::ScrollSticky => "Sticky Sections",
        }
    }

    /// One-line description shown under the label in the selector.
    pub fn blurb(&self) -> &'static str {
        match self {
            HomeLayout::HolyGrail => "Feed flanked by left + right sidebars.",
            HomeLayout::StickySidebar => "Pinned side nav beside a scrolling feed.",
            HomeLayout::SplitScreen => "Inverted intro pane next to the feed.",
            HomeLayout::FullBleed => "Edge-to-edge feed, no chrome.",
            HomeLayout::Drawer => "Off-canvas nav toggled over the feed.",
            HomeLayout::MegaMenu => "Top bar with a drop-down mega panel.",
            HomeLayout::BentoGrid => "Asymmetric tile grid of posts.",
            HomeLayout::Masonry => "Staggered multi-column columns.",
            HomeLayout::CardGrid => "Uniform responsive card grid.",
            HomeLayout::Editorial => "Centered reading measure + aside.",
            HomeLayout::HeroScroll => "Full-screen hero, feed below.",
            HomeLayout::ScrollSticky => "Scrolling posts beside a pinned panel.",
        }
    }
}
