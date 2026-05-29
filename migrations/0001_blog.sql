-- Blog schema. Runs AFTER arium's migrator() (users, roles, sessions, …) and
-- membership_migrator() (arium_resource_members). author_id / uploaded_by /
-- user_id reference arium's `users.id`.

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
    author_id          INTEGER NOT NULL,
    category_id        INTEGER,
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
    post_id INTEGER NOT NULL,
    tag_id  INTEGER NOT NULL,
    PRIMARY KEY (post_id, tag_id)
);
CREATE INDEX IF NOT EXISTS idx_post_tags_tag ON post_tags(tag_id);

CREATE TABLE IF NOT EXISTS comments (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id     INTEGER NOT NULL,
    author_id   INTEGER,           -- NULL for guests
    guest_name  TEXT,
    guest_email TEXT,
    body        TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending', 'approved', 'rejected')),
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_comments_post_status ON comments(post_id, status);

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
    subscriber_id INTEGER NOT NULL,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS media (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    filename    TEXT NOT NULL,
    url         TEXT NOT NULL,
    uploaded_by INTEGER NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS post_views (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    post_id      INTEGER NOT NULL,
    viewed_at    TEXT NOT NULL DEFAULT (datetime('now')),
    referrer     TEXT,
    visitor_hash TEXT
);
CREATE INDEX IF NOT EXISTS idx_post_views_post ON post_views(post_id);

-- Blog-specific profile fields (arium's users already has display_name/avatar_url).
CREATE TABLE IF NOT EXISTS user_profiles (
    user_id      INTEGER PRIMARY KEY,
    bio          TEXT,
    social_links TEXT  -- JSON
);

-- Site-wide key/value settings (e.g. `theme_hue` for the Tailwind brand accent).
CREATE TABLE IF NOT EXISTS site_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Full-text search over title + body, kept in sync from `posts` via triggers.
CREATE VIRTUAL TABLE IF NOT EXISTS posts_fts USING fts5(
    title,
    body,
    content='posts',
    content_rowid='id'
);

CREATE TRIGGER IF NOT EXISTS posts_ai AFTER INSERT ON posts BEGIN
    INSERT INTO posts_fts(rowid, title, body) VALUES (new.id, new.title, new.body_md);
END;
CREATE TRIGGER IF NOT EXISTS posts_ad AFTER DELETE ON posts BEGIN
    INSERT INTO posts_fts(posts_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body_md);
END;
CREATE TRIGGER IF NOT EXISTS posts_au AFTER UPDATE ON posts BEGIN
    INSERT INTO posts_fts(posts_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body_md);
    INSERT INTO posts_fts(rowid, title, body) VALUES (new.id, new.title, new.body_md);
END;
