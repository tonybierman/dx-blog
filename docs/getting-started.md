# Getting started

## Prerequisites

- A recent stable Rust toolchain and the wasm target: `rustup target add wasm32-unknown-unknown`
- The Dioxus CLI: `cargo install dioxus-cli`

> Auth/authz comes from [arium](https://github.com/tonybierman/arium), pulled in as a git
> dependency — Cargo fetches it automatically, no separate checkout needed.

## Run

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
