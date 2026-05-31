# riparion-cms tasks. Run `just` to list recipes.

# Optional, gitignored local recipes (`?` = skipped when the file is absent, so
# contributors/CI without it aren't broken). Add personal recipes to local.just.
import? 'local.just'

default:
    @just --list

# Clear dx's web-bundle output (target/dx). Use after an arium/dep bump when
# arium components render unstyled — dx reuses stale content-addressed CSS
# assets whose css_module hashes no longer match the recompiled components.
# Leaves the cargo target/debug cache intact, so the next build is mostly
# re-link + asset regen, not a full recompile.
clean-assets:
    rm -rf target/dx

# Clean the stale assets, then serve fresh.
fresh: clean-assets
    dx serve

# Serve normally (no clean).
serve:
    dx serve

# seed::run_if_empty no-ops while any post exists, so a reseed must start from an
# empty DB. Set DX_SEED_ADMIN_PASSWORD / DX_SEED_DEMO_PASSWORD to choose
# passwords, else they're generated and printed once.
# Wipe the dev DB (data/blog.db) and serve with seeding on. Destructive, dev only.
reseed:
    rm -f data/blog.db data/blog.db-wal data/blog.db-shm
    DX_SEED=1 dx serve

# Serve a release client — far smaller/faster wasm than debug, so no
# slow-to-hydrate window on fresh loads. Slower first compile.
serve-release:
    dx serve --release

# Clean the stale assets, then serve release.
fresh-release: clean-assets
    dx serve --release
