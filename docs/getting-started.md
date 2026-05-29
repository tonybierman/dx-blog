# Getting started

## Prerequisites

- A recent stable Rust toolchain and the wasm target: `rustup target add wasm32-unknown-unknown`
- The Dioxus CLI: `cargo install dioxus-cli`

> Auth/authz comes from [arium](https://github.com/tonybierman/arium), pulled in as a git
> dependency — Cargo fetches it automatically, no separate checkout needed.

## Run

```sh
cp .env.example .env      # all values are optional in dev
# Uncomment DX_SEED=1 (and, for local convenience, DX_AUTH_SKIP_EMAIL_VERIFICATION=1)
dx serve                  # builds the client, compiles tailwind.css, and starts the server
```

The app comes up at <http://localhost:8080>. With `DX_SEED=1` set, a fresh database
is seeded with demo content and three accounts:

```
admin@example.com   ada@example.com   linus@example.com
```

The two demo authors (`ada@` / `linus@`) use the password `password`. The admin
account is fully privileged, so it never gets that public password: it uses
`DX_SEED_ADMIN_PASSWORD` if you set one, otherwise a random password printed to
the console once at seed time. Seeding only runs when `DX_SEED=1` is set
explicitly — a build without it (e.g. a deploy) never plants demo accounts.

> `assets/tailwind.css` is a generated artifact (ignored by git) — `dx serve` / `dx build`
> regenerate it from `tailwind.css`. To build it by hand: `npx @tailwindcss/cli -i tailwind.css -o assets/tailwind.css`.
