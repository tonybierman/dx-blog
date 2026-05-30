# Running in Docker

The repo ships a multi-stage [`Dockerfile`](../Dockerfile) and a
[`docker-compose.yml`](../docker-compose.yml) that build a production web bundle
and run it in a slim container.

## Prerequisites

- Docker with BuildKit (the default in current Docker; the build uses cache mounts).
- Nothing else — the build is hermetic. The builder image installs the Rust
  toolchain, the wasm target, and the Dioxus CLI itself, and compiles Tailwind
  (dx has it built in — no Node needed).

## Quick start

```sh
DOCKER_BUILDKIT=1 docker compose up --build
```

The app comes up at <http://localhost:8888>. The first build is slow (it
compiles the Dioxus CLI, the wasm client, and the whole server, including the
`arium` git dependency); later rebuilds reuse cached layers and cargo caches.

To seed demo content and accounts on the first run, see
[First run & seeding](#first-run--seeding) below.

## How the image is built

The `Dockerfile` has two stages:

1. **Builder** (`rust:1.95-bookworm`) — adds the `wasm32-unknown-unknown`
   target, installs `dioxus-cli@0.7.9`, and runs:

   ```sh
   dx bundle --platform web --fullstack --release
   ```

   This compiles the hydrated wasm client, compiles `tailwind.css` into the
   hashed `public/assets/tailwind-*.css`, and links the axum server binary. The
   artifacts land in `target/dx/dx-blog/release/web/` (a `server` binary and a
   `public/` directory) and are copied to `/out`.

2. **Runtime** (`debian:bookworm-slim`) — copies `/out` to `/app` and adds
   `ca-certificates` (for outbound TLS — the binary uses rustls, so no libssl is
   needed). The bundled server resolves `./public` relative to its own location,
   so `server` and `public/` stay siblings under `/app`.

`assets/tailwind.css` is **not** a runtime mount — with the production bundle it
is compiled at build time and baked into `public/` as a content-hashed asset
that the server serves directly.

## Persistent data (volumes)

`docker-compose.yml` bind-mounts two host directories so state survives
container rebuilds:

| Host path  | Container path | Holds |
|------------|----------------|-------|
| `./data`   | `/app/data`    | the SQLite database (`blog.db`) |
| `./uploads`| `/app/uploads` | media-library uploads, served at `/uploads` |

`DATABASE_URL` is set to `sqlite:///app/data/blog.db?mode=rwc` to match the data
volume; `mode=rwc` creates the file on first run.

> **Ownership note:** the container runs as a non-root user pinned to
> **UID/GID 1000** (the usual desktop default), so files it writes to `./data`
> and `./uploads` are owned by your host user. If your host UID differs, build
> with it:
>
> ```sh
> UID=$(id -u) GID=$(id -g) docker compose build
> ```

## Configuration

Environment variables are set in the `environment:` block of
`docker-compose.yml`. A few are wired to pass through from your shell so you can
set them per-run without editing the file:

| Variable | Purpose |
|---|---|
| `SITE_URL` | Pass-through. Canonical origin for absolute URLs in `/sitemap.xml` and `/feed.xml`. Set to your real https origin behind a reverse proxy; defaults to the local published port. |
| `DX_SEED` | Pass-through. `1` seeds demo content/accounts on an empty DB. |
| `DX_SEED_ADMIN_PASSWORD` | Pass-through. Password for the seeded `admin@example.com`; random (printed to logs) if unset. |
| `DX_AUTH_BOOTSTRAP_ADMIN_EMAIL` | Pass-through. First account to register with this email becomes admin. |

See [configuration.md](configuration.md) and [`.env.example`](../.env.example)
for the full list (SMTP, GitHub OAuth, etc.); add any of them to the
`environment:` block as needed.

## First run & seeding

`DX_SEED` is read at **server startup**, not at build time — it cannot be passed
to `docker build`. Seed by setting it on the `up` command against an empty DB:

```sh
# Seed with a known admin password:
DX_SEED=1 DX_SEED_ADMIN_PASSWORD=secret docker compose up --build
```

Seeding only runs when the database is empty (the seeder no-ops once any post
exists). Because `./data` persists, the seed runs once and not on later starts.
To re-seed from scratch, remove the DB first:

```sh
docker compose down
rm -f data/blog.db data/blog.db-wal data/blog.db-shm
DX_SEED=1 docker compose up
```

The seeded demo authors (`ada@` / `linus@example.com`) use the password
`password`. The admin password is whatever you pinned with
`DX_SEED_ADMIN_PASSWORD`, or a random value printed once to the logs:

```sh
docker compose logs blog | grep -i password
```

## Common commands

```sh
docker compose up --build         # build + run in the foreground
docker compose up -d --build      # build + run detached
docker compose logs -f blog       # follow logs
docker compose down               # stop and remove the container
docker compose build --no-cache   # force a clean rebuild
```
