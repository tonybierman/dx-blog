# dx-blog — Next Steps

The core app is built and runs end-to-end (auth via arium, public reader, admin
authoring, seed data, FTS search). This is the backlog of follow-ups, grouped by
theme. Check items off as they land.

## Auth & accounts
- [x] Add an **Account** route (`/account`) wrapping arium's `AccountSettings` (change password, display name, delete account); linked from the header
- [x] Wire **MFA** routes/components — `MfaSetup` embedded on `/account`; `MfaChallenge` handles `LoginOutcome::MfaRequired` in the login flow
- [ ] Verify the **forgot-password / reset / verify-email** flows end-to-end with a real SMTP mailer (set `SMTP_*` in `.env`)
- [x] Let authors **edit their own post from the post detail page** via `ResourceGate` (edit affordance), not just the admin table
- [x] Author-facing **draft preview** (drafts are currently invisible on public pages even to their author)

## Reader experience
- [x] Home right sidebar: real **featured posts** (most-viewed) + **recent comments** (replaced the placeholder "About" box)
- [ ] Search **facets** (category / tag / date) in the right sidebar, beyond the text box
- [x] **Pagination** for tag / author / archive feeds (currently single-page / unbounded)
- [ ] Subscriber **double opt-in** confirmation flow (the `confirmed` column exists but isn't exercised)
- [x] Empty-state and loading **skeletons** instead of plain "Loading…" (reader feeds, home, post detail)

## Authoring & admin
- [x] Featured-image picker that **selects from the media library** (URL field + "Library" thumbnail picker + preview)
- [x] Debounce the editor's **live-preview** server round-trip (fires on every keystroke today) — 400ms via `dioxus-sdk-time`
- [x] Category/tag **edit** (rename), not just create/delete, in Settings (slug kept stable)
- [ ] Sort/filter controls on the admin **post table** (PLAN asks for sortable/filterable)
- [ ] Real **analytics** beyond counts: top referrers, views-over-time

## Visual polish
- [ ] Decide on Tailwind **typography plugin** vs the hand-rolled `.prose` CSS in `assets/main.css`
- [ ] Confirm Tailwind utility classes are compiling in `dx serve` and pass a responsive pass (mobile sidebars, bento/masonry)
- [ ] Consistent theming between arium's auth UI CSS and the blog's Tailwind look

## Quality & ops
- [ ] **Commit** the work (nothing is committed yet) on a branch + open a PR
- [ ] Add **tests**: server-fn round-trips (list/create/authz), slug uniqueness, markdown render/sanitize, comment auto-approve
- [ ] Real **multipart upload** endpoint (current upload is base64 via a server fn — works but heavier)
- [x] Server-side **error boundary** so runtime errors route to `/500` instead of a blank state
- [ ] Reseed note: `rm data/blog.db` then restart to pick up seed-content changes (e.g. the de-duplicated post titles)
- [ ] Production config review: secure cookies, `DATABASE_URL`, disable `DX_AUTH_SKIP_EMAIL_VERIFICATION`, set a bootstrap admin

## Known gotchas (already handled — keep in mind)
- [x] Server fns with **arguments must be `#[post]`**, not `#[get]` (GET can't carry a body)
- [x] Route-param pages must use **`use_reactive!`** in `use_resource` so they refetch when the param changes
- [x] Blog schema runs as **idempotent raw SQL**, not a tracked sqlx migrator (avoids clashing with arium's `_sqlx_migrations`)
- [x] `data/` dir must exist before opening SQLite (created at startup)
