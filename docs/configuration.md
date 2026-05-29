# Configuration

All configuration is via environment variables; see [`.env.example`](../.env.example) for the
full list. Common ones:

| Variable | Purpose |
|---|---|
| `DATABASE_URL` | SQLite location (defaults to `./data/blog.db`) |
| `SITE_URL` | Canonical origin for absolute URLs in the sitemap and feed |
| `SITE_TITLE` | Title shown in the Atom feed |
| `DX_AUTH_SKIP_EMAIL_VERIFICATION` | Skip the email round-trip in dev |
| `GITHUB_CLIENT_ID` / `GITHUB_CLIENT_SECRET` | Enable GitHub OAuth |
| `SMTP_*` | Outgoing mail (password reset, verification, subscriptions) |
