# dx-blog tasks. Run `just` to list recipes.

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

# Serve a release client — far smaller/faster wasm than debug, so no
# slow-to-hydrate window on fresh loads. Slower first compile.
serve-release:
    dx serve --release

# Clean the stale assets, then serve release.
fresh-release: clean-assets
    dx serve --release
