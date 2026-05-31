# syntax=docker/dockerfile:1

# ----------------------------------------------------------------------------
# Stage 1 — build a release web bundle (server binary + hashed public assets).
#
# `dx bundle` compiles the wasm client, compiles Tailwind (built into dx — no
# Node needed; it reads ./tailwind.css and emits the hashed assets/tailwind-*.css
# into public/), and links the axum server binary. arium is a git dependency, so
# the builder needs git + a network during the build.
# ----------------------------------------------------------------------------
FROM rust:1.95-bookworm AS builder

# wasm target for the hydrated client half of the fullstack build.
RUN rustup target add wasm32-unknown-unknown

# The Dioxus CLI that drives the bundle. Pinned to the version this repo uses.
RUN cargo install dioxus-cli@0.7.9 --locked

WORKDIR /app
COPY . .

# Cache mounts keep the cargo registry/git checkouts and the target dir warm
# across rebuilds (BuildKit). The target dir lives in a cache mount, so the
# bundle artifacts are copied out to /out within the same RUN — anything left in
# the cache mount is NOT present in the image layer.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    dx bundle --platform web --fullstack --release \
 && mkdir -p /out \
 && cp -a target/dx/riparion-cms/release/web/. /out/

# ----------------------------------------------------------------------------
# Stage 2 — slim runtime. Only the server binary + public/ ship here.
#
# The bundled server resolves ./public relative to its own location, so server
# and public/ stay siblings under /app. ca-certificates is needed for outbound
# TLS (SMTP / GitHub OAuth); the binary uses rustls, so no libssl is required.
# ----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# Run as a non-root user pinned to UID/GID 1000 so files written to the
# bind-mounted ./data and ./uploads volumes are owned by the host user (the
# default desktop UID), not root. Override at build time if your host UID
# differs: docker compose build --build-arg UID=$(id -u) --build-arg GID=$(id -g)
ARG UID=1000
ARG GID=1000
RUN groupadd --gid "$GID" app \
 && useradd --uid "$UID" --gid "$GID" --no-create-home --shell /usr/sbin/nologin app

WORKDIR /app
# --chown so the binary/assets and the volume mount points are owned by `app`.
# The data/ and uploads/ dirs are pre-created so fresh named volumes inherit
# the right ownership (bind mounts use the host directory's ownership instead).
COPY --from=builder --chown=$UID:$GID /out/ /app/
RUN mkdir -p /app/data /app/uploads && chown -R "$UID:$GID" /app
USER app

# Bind to all interfaces inside the container; the SQLite DB lives on a mounted
# volume (see docker-compose.yml). Uploaded media is written to ./uploads
# (cwd-relative), also a mounted volume.
ENV IP=0.0.0.0 \
    PORT=8888 \
    DATABASE_URL=sqlite:///app/data/blog.db?mode=rwc

EXPOSE 8888
CMD ["/app/server"]
