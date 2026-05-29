# Full-Featured Blog Application — Build Prompt

## Overview

Build a full-featured blog application with a React frontend (React Router v6) and a Node.js/Express backend with a SQLite database. The app has three distinct user roles: **visitor**, **author**, and **admin**.

---

## Tech Stack

- **Frontend** and **Backend**: Dioxus Fullstack
- **Database**: SQLite
- **Auth**: ../arium

---

## Layout System

The app uses four structural layout components; bring them in from dioxus-mcp. Build each as a reusable wrapper:

| Layout | Description |
|---|---|
| `HolyGrailLayout` | Persistent top nav, optional left sidebar, optional right sidebar, main content area, footer |
| `FullBleedLayout` | No sidebars, no persistent chrome — content fills the viewport |
| `BentoGridLayout` | Asymmetric card tile grid; first card spans 2 columns |
| `MasonryLayout` | Staggered multi-column grid; cards vary in height naturally |

All layouts must be fully responsive using CSS Grid or Flexbox. No hardcoded breakpoints where intrinsic sizing (`minmax`, `clamp`, `auto-fit`) can handle it instead.

---

## Routes & Layouts

### Auth (FullBleedLayout)

| Route | Page | Notes |
|---|---|---|
| `/login` | Login | Email + password form, redirect to `/` on success |
| `/register` | Register | Username, email, password |
| `/forgot-password` | Forgot Password | Email input, sends reset link |

### Public / Reader

| Route | Layout | Page | Notes |
|---|---|---|---|
| `/` | HolyGrail | Home Feed | Left sidebar: tag list + categories. Right sidebar: featured posts, recent comments. Main: paginated post cards |
| `/post/:slug` | FullBleed | Post Detail | Full article, author bio at bottom, comment section |
| `/category/:slug` | HolyGrail | Category Feed | Same shell as home, filtered by category |
| `/tag/:slug` | BentoGrid | Tag Feed | Visual tile grid of posts with this tag |
| `/author/:slug` | HolyGrail | Author Profile | Left sidebar: author bio + social links. Main: author's posts |
| `/archive` | Masonry | Archive | All posts across all time, staggered cards |
| `/search` | HolyGrail | Search Results | Right sidebar: facets (category, tag, date). Main: results |
| `/subscribe` | FullBleed | Subscribe | Email capture form, no other chrome |

### Admin (requires `admin` role)

| Route | Layout | Page | Notes |
|---|---|---|---|
| `/admin` | HolyGrail | Dashboard | Left sidebar: admin nav. Main: stats (post count, comment count, subscriber count, recent activity) rendered as a BentoGrid of metric tiles |
| `/admin/posts` | HolyGrail | Post List | Sortable, filterable table of all posts (draft, published, archived) |
| `/admin/posts/new` | FullBleed | New Post | Full-width rich text editor, title, slug (auto-generated), category, tags, featured image, publish/draft toggle |
| `/admin/posts/:id/edit` | FullBleed | Edit Post | Same as new post, pre-populated |
| `/admin/media` | Masonry | Media Library | Uploaded images in a staggered grid; click to copy URL |
| `/admin/comments` | HolyGrail | Comment Moderation | Queue of pending comments; approve / reject / delete |
| `/admin/users` | HolyGrail | User Management | Table of users; edit roles (visitor / author / admin) |
| `/admin/settings` | HolyGrail | Site Settings | Left sidebar: settings sections (general, SEO, email). Main: form for the selected section |
| `/admin/analytics` | BentoGrid | Analytics | Tiles: page views, unique visitors, top posts, top referrers |

### Error Pages (FullBleedLayout)

| Route | Page |
|---|---|
| `/404` | Not Found |
| `/500` | Server Error |

---

## Data Models

### User
```
id, username, email, password_hash, role (visitor|author|admin),
bio, avatar_url, social_links (JSON), created_at
```

### Post
```
id, title, slug, body (rich text/HTML), excerpt, author_id,
category_id, featured_image_url, status (draft|published|archived),
published_at, created_at, updated_at
```

### Category
```
id, name, slug, description
```

### Tag
```
id, name, slug
```

### PostTag (join)
```
post_id, tag_id
```

### Comment
```
id, post_id, author_id (nullable for guests), guest_name,
guest_email, body, status (pending|approved|rejected), created_at
```

### Subscriber
```
id, email, confirmed (boolean), created_at
```

### Media
```
id, filename, url, uploaded_by, created_at
```

---

## API Endpoints

### Auth
- `POST /api/auth/register`
- `POST /api/auth/login` — returns JWT in httpOnly cookie
- `POST /api/auth/logout`
- `POST /api/auth/refresh`
- `POST /api/auth/forgot-password`
- `POST /api/auth/reset-password`

### Posts
- `GET /api/posts` — paginated, filterable by category/tag/status
- `GET /api/posts/:slug`
- `POST /api/posts` — author/admin only
- `PUT /api/posts/:id` — author (own posts) / admin
- `DELETE /api/posts/:id` — admin only

### Categories & Tags
- `GET /api/categories`
- `GET /api/tags`
- CRUD endpoints for admin

### Comments
- `GET /api/posts/:id/comments` — approved only for public
- `POST /api/posts/:id/comments`
- `PUT /api/comments/:id` — moderation (admin)
- `DELETE /api/comments/:id` — admin

### Users (admin only)
- `GET /api/users`
- `PUT /api/users/:id`
- `DELETE /api/users/:id`

### Media
- `POST /api/media/upload` — multipart, author/admin only
- `GET /api/media`
- `DELETE /api/media/:id`

### Subscribers
- `POST /api/subscribers`
- `GET /api/subscribers` — admin only

### Analytics (admin only)
- `GET /api/analytics/summary` — aggregate counts
- `GET /api/analytics/top-posts`

---

## Auth & Access Control

- Protect all `/admin/*` routes: redirect to `/login` if not authenticated or not admin
- Authors can create posts and edit their own; admins can edit any
- Comments default to `pending`; auto-approve if the author is a logged-in user with prior approved comments
- Guests can comment with name + email

---

## Seed Data

Include a seed script that creates:
- 1 admin user
- 2 author users
- 4 categories
- 10 tags
- 20 posts (mix of draft and published, spread across authors/categories/tags)
- 30 comments (mix of pending and approved)
- 5 subscribers

---

## Additional Requirements

- **Slug generation**: auto-generate post slugs from titles; ensure uniqueness
- **Pagination**: cursor-based or offset; default 10 posts per page
- **Search**: basic full-text search on title and body (SQLite FTS5)
- **Image upload**: store locally under `/uploads`; serve as static files
- **Rich text**: store as HTML; sanitize on save with DOMPurify or equivalent
- **Error handling**: consistent JSON error shape `{ error: { code, message } }`
- **Environment config**: `.env` for secrets
