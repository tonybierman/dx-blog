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
- Admin-selectable home-page layout — 12 structural kinds (holy-grail, bento, masonry, editorial, hero, and more)

**Authoring & admin**
- Markdown editor with debounced live preview; HTML is rendered and sanitized server-side
- Draft / published / archived workflow, with author-facing draft preview
- Media library with a featured-image picker
- Admin dashboard: sortable/filterable post table, comment moderation, users
- Settings split into Settings (site title + tagline), Appearance (theme + home layout), and Taxonomy (categories + tags)
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
| Auth/authz | [`arium-dioxus`](https://github.com/tonybierman/arium) (git dependency) |
| Styling | Tailwind CSS v4 |
| Markdown | `pulldown-cmark` + `ammonia` sanitizer |

## Documentation

- [Getting started](docs/getting-started.md) — prerequisites and running locally
- [Configuration](docs/configuration.md) — environment variables
- [Development](docs/development.md) — checking the two build targets and running tests

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for
inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed
as above, without any additional terms or conditions.
