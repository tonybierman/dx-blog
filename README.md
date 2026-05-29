# dx-blog

A full-featured blog built on [Dioxus](https://dioxuslabs.com) Fullstack (Rust), with
authentication and authorization provided by [arium](https://github.com/tonybierman/arium).
Server-rendered and hydrated, SQLite-backed, styled with Tailwind v4.

## Features

**Reading**
- Paginated home feed, category / tag / author feeds, and a full archive
- Full-text search with category / tag / date facets
- Per-post pages with comments, author bios, and related metadata
- Loading skeletons and empty states; responsive layouts (holy-grail, bento, masonry, full-bleed)

**Authoring & admin**
- Markdown editor with debounced live preview; HTML is rendered and sanitized server-side
- Draft / published / archived workflow, with author-facing draft preview
- Media library with a featured-image picker
- Admin dashboard: sortable/filterable post table, comment moderation, users, settings
- Analytics: view counts, 30-day views-over-time, and top referrers
- One-knob theming — the whole accent palette is driven by a single brand hue

**Accounts** (via arium)
- Email/password and GitHub OAuth sign-in, MFA, password reset, email verification
- Per-post ownership using arium's resource-membership model: authors edit their own posts; admins edit any
- Account settings (display name, password, delete account)

**Syndication & SEO**
- Dynamic `/sitemap.xml` (posts, categories, tags, authors)
- Atom feed at `/feed.xml`, with autodiscovery
- Double opt-in email subscriptions

## Tech stack

| | |
|---|---|
| UI + server | Dioxus 0.7 Fullstack (axum under the hood) |
| Database | SQLite via `sqlx` |
| Auth/authz | `arium-dioxus` (local workspace dependency) |
| Styling | Tailwind CSS v4 |
| Markdown | `pulldown-cmark` + `ammonia` sanitizer |

## Getting started

### Prerequisites

- A recent stable Rust toolchain and the wasm target: `rustup target add wasm32-unknown-unknown`
- The Dioxus CLI: `cargo install dioxus-cli`
- **arium** checked out next to this repo — it's a path dependency (`../arium/crates/arium-dioxus`):

  ```sh
  git clone https://github.com/tonybierman/arium ../arium
  ```

### Run

```sh
cp .env.example .env      # all values are optional in dev
dx serve                  # builds the client, compiles tailwind.css, and starts the server
```

The app comes up at <http://localhost:8080>. On a fresh database it auto-seeds demo
content and three accounts (all password `password`):

```
admin@example.com   ada@example.com   linus@example.com
```

> `assets/tailwind.css` is a generated artifact (ignored by git) — `dx serve` / `dx build`
> regenerate it from `tailwind.css`. To build it by hand: `npx @tailwindcss/cli -i tailwind.css -o assets/tailwind.css`.

## Configuration

All configuration is via environment variables; see [`.env.example`](.env.example) for the
full list. Common ones:

| Variable | Purpose |
|---|---|
| `DATABASE_URL` | SQLite location (defaults to `./data/blog.db`) |
| `SITE_URL` | Canonical origin for absolute URLs in the sitemap and feed |
| `SITE_TITLE` | Title shown in the Atom feed |
| `DX_AUTH_SKIP_EMAIL_VERIFICATION` | Skip the email round-trip in dev |
| `GITHUB_CLIENT_ID` / `GITHUB_CLIENT_SECRET` | Enable GitHub OAuth |
| `SMTP_*` | Outgoing mail (password reset, verification, subscriptions) |

## Development

The crate compiles to two targets that a plain `cargo check` can't cover at once. Check each:

```sh
cargo check --no-default-features --features server,sqlite              # server
cargo check --no-default-features --features web --target wasm32-unknown-unknown  # client
```

Server-side tests run with:

```sh
cargo test --no-default-features --features server,sqlite
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed
as above, without any additional terms or conditions.
