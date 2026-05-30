-- Blog schema. Runs AFTER arium's migrator() (users, roles, sessions, …) and
-- membership_migrator() (arium_resource_members). author_id / uploaded_by /
-- user_id reference arium's `users.id`.
--
-- Foreign keys: enforcement is per-connection and enabled in main.rs via
-- SqliteConnectOptions::foreign_keys(true). The REFERENCES … ON DELETE clauses
-- below only bind on a freshly created table — SQLite can't ALTER a constraint
-- onto an existing one — so databases created before these were added keep
-- relying on the hand-rolled cascades in server::admin::delete_post. References
-- into arium's user-owned tables use CASCADE / SET NULL so arium's own
-- account-deletion flow isn't blocked by a dangling blog row.

CREATE TABLE IF NOT EXISTS categories (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    slug        TEXT NOT NULL UNIQUE,
    description TEXT
);

CREATE TABLE IF NOT EXISTS tags (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS posts (
    id                 INTEGER PRIMARY KEY AUTOINCREMENT,
    title              TEXT NOT NULL,
    slug               TEXT NOT NULL UNIQUE,
    body_md            TEXT NOT NULL DEFAULT '',
    body_html          TEXT NOT NULL DEFAULT '',
    excerpt            TEXT NOT NULL DEFAULT '',
    author_id          INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    category_id        INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    featured_image_url TEXT,
    status             TEXT NOT NULL DEFAULT 'draft'
                         CHECK (status IN ('draft', 'published', 'archived')),
    published_at       TEXT,
    created_at         TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at         TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_posts_status_published ON posts(status, published_at);
CREATE INDEX IF NOT EXISTS idx_posts_author ON posts(author_id);
CREATE INDEX IF NOT EXISTS idx_posts_category ON posts(category_id);

CREATE TABLE IF NOT EXISTS post_tags (
    post_id INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    tag_id  INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (post_id, tag_id)
);
CREATE INDEX IF NOT EXISTS idx_post_tags_tag ON post_tags(tag_id);

CREATE TABLE IF NOT EXISTS comments (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id     INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    author_id   INTEGER REFERENCES users(id) ON DELETE SET NULL,  -- NULL for guests
    guest_name  TEXT,
    guest_email TEXT,
    body        TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending', 'approved', 'rejected')),
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_comments_post_status ON comments(post_id, status);
-- Home "recent approved comments" reads filter by status and order by created_at
-- across all posts; this covers that path (idx_comments_post_status is keyed on
-- post_id first, so it can't serve the post-agnostic recent query).
CREATE INDEX IF NOT EXISTS idx_comments_status_created ON comments(status, created_at);

CREATE TABLE IF NOT EXISTS subscribers (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    email      TEXT NOT NULL UNIQUE,
    confirmed  INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Double opt-in confirmation tokens. A fresh subscribe replaces any pending
-- token for that subscriber; confirming consumes (deletes) the row.
CREATE TABLE IF NOT EXISTS subscriber_tokens (
    token         TEXT PRIMARY KEY,
    subscriber_id INTEGER NOT NULL REFERENCES subscribers(id) ON DELETE CASCADE,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
-- Look up / delete a subscriber's tokens by subscriber_id (subscribe + confirm).
CREATE INDEX IF NOT EXISTS idx_subscriber_tokens_subscriber
    ON subscriber_tokens(subscriber_id);

CREATE TABLE IF NOT EXISTS media (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    filename    TEXT NOT NULL,
    url         TEXT NOT NULL,
    uploaded_by INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Resized, modern-format renditions of an uploaded image (WordPress-style:
-- thumb/small/medium/large + a full-size re-encode), generated on upload. The
-- original file stays the canonical `media.url`; these are the responsive
-- `srcset` sources. `label` is the size bucket, `format` the codec (webp/avif),
-- and (width,height) the pixel dimensions for that rendition. Rows cascade with
-- the parent media row; the files on disk are unlinked separately on delete.
CREATE TABLE IF NOT EXISTS media_variants (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    media_id   INTEGER NOT NULL REFERENCES media(id) ON DELETE CASCADE,
    label      TEXT NOT NULL,
    format     TEXT NOT NULL,
    width      INTEGER NOT NULL,
    height     INTEGER NOT NULL,
    url        TEXT NOT NULL,
    bytes      INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
-- Look up all renditions for a media row (build srcset; unlink files on delete).
CREATE INDEX IF NOT EXISTS idx_media_variants_media ON media_variants(media_id);

CREATE TABLE IF NOT EXISTS post_views (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id      INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    viewed_at    TEXT NOT NULL DEFAULT (datetime('now')),
    referrer     TEXT,
    visitor_hash TEXT
);
CREATE INDEX IF NOT EXISTS idx_post_views_post ON post_views(post_id);
-- The per-view 24h dedup probe in analytics::record_view filters on
-- (post_id, visitor_hash, viewed_at); this composite lets it seek instead of
-- scanning a post's whole view history on every page view.
CREATE INDEX IF NOT EXISTS idx_post_views_dedup
    ON post_views(post_id, visitor_hash, viewed_at);
-- views-over-time filters by viewed_at alone across all posts.
CREATE INDEX IF NOT EXISTS idx_post_views_viewed_at ON post_views(viewed_at);

-- Anonymous reactions/claps. Like post_views, rows are append-only and keyed by
-- the coarse visitor_hash (analytics::visitor_hash) — no account required. Each
-- row is one clap; reactions::add_reaction caps per-visitor totals and bursts.
CREATE TABLE IF NOT EXISTS reactions (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id      INTEGER NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    kind         TEXT NOT NULL DEFAULT 'clap',
    visitor_hash TEXT NOT NULL,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_reactions_post ON reactions(post_id);
-- add_reaction's per-visitor cap + burst probes filter on
-- (post_id, visitor_hash, created_at); this composite lets them seek.
CREATE INDEX IF NOT EXISTS idx_reactions_dedup
    ON reactions(post_id, visitor_hash, created_at);

-- Blog-specific profile fields (arium's users already has display_name/avatar_url).
CREATE TABLE IF NOT EXISTS user_profiles (
    user_id      INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    bio          TEXT,
    social_links TEXT  -- JSON
);

-- Site-wide key/value settings (e.g. `theme_hue` for the Tailwind brand accent).
CREATE TABLE IF NOT EXISTS site_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Full-text search over title + body, kept in sync from `posts` via triggers.
-- The FTS columns must be NAMED after the `posts` columns they shadow: with
-- content='posts', a `rebuild` reads each FTS column from the same-named source
-- column, so the body column has to be `body_md` (posts has no `body`). The
-- triggers below feed `body_md` accordingly.
CREATE VIRTUAL TABLE IF NOT EXISTS posts_fts USING fts5(
    title,
    body_md,
    content='posts',
    content_rowid='id'
);

CREATE TRIGGER IF NOT EXISTS posts_ai AFTER INSERT ON posts BEGIN
    INSERT INTO posts_fts(rowid, title, body_md) VALUES (new.id, new.title, new.body_md);
END;
CREATE TRIGGER IF NOT EXISTS posts_ad AFTER DELETE ON posts BEGIN
    INSERT INTO posts_fts(posts_fts, rowid, title, body_md) VALUES ('delete', old.id, old.title, old.body_md);
END;
CREATE TRIGGER IF NOT EXISTS posts_au AFTER UPDATE ON posts BEGIN
    INSERT INTO posts_fts(posts_fts, rowid, title, body_md) VALUES ('delete', old.id, old.title, old.body_md);
    INSERT INTO posts_fts(rowid, title, body_md) VALUES (new.id, new.title, new.body_md);
END;
