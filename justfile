set shell := ["bash", "-cu"]

_classic_args    := "--target xtensa-esp32-none-elf --release -Zbuild-std=core,alloc"
# RUSTFLAGS for ESP targets: -D warnings PLUS the linker script that .cargo/config.toml provides
# but that env RUSTFLAGS= would otherwise override.
_esp_rustflags   := "-D warnings -C link-arg=-Tlinkall.x"
_ballet_rustflags := "-D warnings -A long-running-const-eval"
_ballet_esp_rustflags := _esp_rustflags + " -A long-running-const-eval"

# ── Tests / checks ───────────────────────────────────────────────────────────

# Run linkage-blaze-core tests (unit tests + doc tests + alloc integration tests)
test-core:
    env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-core
    env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-core --features alloc

# Check and build all crates
check-all:
    env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-core
    env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-core --features alloc
    source ~/export-esp.sh && env RUSTFLAGS="{{_esp_rustflags}}" cargo +esp build -p linkage-blaze-classic --example armatron {{_armatron_args}}
    source ~/export-esp.sh && env RUSTFLAGS="{{_esp_rustflags}}" cargo +esp build -p linkage-blaze-classic --example clock {{_clock_args}}
    source ~/export-esp.sh && env RUSTFLAGS="{{_esp_rustflags}}" cargo +esp build -p linkage-blaze-classic --example skeleton-clock {{_skeleton_clock_args}}
    source ~/export-esp.sh && env RUSTFLAGS="{{_ballet_esp_rustflags}}" cargo +esp build -p linkage-blaze-classic --example ballet {{_ballet_args}}
    env RUSTFLAGS="-D warnings" wasm-pack build crates/linkage-blaze-editor --target web --out-dir www/pkg --out-name linkage_blaze_editor

# Alias for check-all
build:
    just check-all

# Build the static GitHub Pages artifact with immutable demo version URLs.
build-pages demo='':
    cargo run --quiet -p linkage-blaze-xtask -- build-pages "{{demo}}"

_pages_port := "8090"

# Build and serve the local Pages gallery for browser review.
run-all-wasm port=_pages_port:
    just build-pages
    cd target/pages && python3 ../../.tools/no_cache_http_server.py {{port}} --next-free

# Dispatch the GitHub Pages workflow on GitHub.
publish-pages ref='main':
    gh workflow run pages.yml --ref "{{ref}}"

# Freeze the current live web assets for one demo into a new immutable Pages version.
bump-demo-version demo version='':
    cargo run --quiet -p linkage-blaze-xtask -- bump-demo-version "{{demo}}" "{{version}}"

# Freeze the current gallery page (/demos/) into a new immutable Pages version.
bump-gallery-version version='':
    cargo run --quiet -p linkage-blaze-xtask -- bump-gallery-version "{{version}}"

# Generate docs and open in browser
docs:
    env RUSTFLAGS="-D warnings" cargo doc -p linkage-blaze-core --no-deps --features alloc --open

# Show generated docs
show-docs:
    just docs

# Bundle docs/context for an outside AI
bundle-docs:
    just _bundle-docs

# Generate rustdoc and bundle repo docs/context for an outside AI
_bundle-docs:
    #!/usr/bin/env bash
    set -euo pipefail
    out_dir="target/ai-docs"
    rustdoc_dir="$out_dir/rustdoc"
    bundle="$out_dir/linkage-blaze-ai-docs.md"
    archive="target/linkage-blaze-ai-docs.tar"

    rm -rf "$out_dir" "$archive"
    mkdir -p "$rustdoc_dir"

    env RUSTFLAGS="-D warnings" cargo doc -p linkage-blaze-core --no-deps --features alloc

    cp -R target/doc/linkage_blaze_core "$rustdoc_dir/"
    cp target/doc/crates.js target/doc/help.html target/doc/search-index.js target/doc/settings.html target/doc/src-files.js "$rustdoc_dir/" 2>/dev/null || true

    {
        printf -- '# linkage-blaze AI docs bundle\n\n'
        printf -- 'Generated: %s UTC\n\n' "$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
        printf -- 'This bundle is intended for an outside AI reviewer. It includes repository guidance, Markdown docs, Cargo manifests, and generated rustdoc HTML copied under `rustdoc/`.\n\n'
        printf -- '## Rustdoc entry points\n\n'
        printf -- '%s\n\n' '- `rustdoc/linkage_blaze_core/index.html`'
        printf -- '## Repository docs and manifests\n\n'
    } > "$bundle"

    find . \
        -path './.git' -prune -o \
        -path './target' -prune -o \
        -path './node_modules' -prune -o \
        -type f \( -name '*.md' -o -name 'Cargo.toml' \) -print \
        | sort \
        | while read -r path; do
            clean_path="${path#./}"
            {
                printf -- '\n## `%s`\n\n' "$clean_path"
                printf -- '```text\n'
                sed 's/```/` ` `/g' "$path"
                printf -- '\n```\n'
            } >> "$bundle"
        done

    tar -cf "$archive" -C target ai-docs
    printf -- 'Wrote %s\n' "$bundle"
    printf -- 'Wrote %s\n' "$archive"

# ── linkage-blaze-classic examples (dance, ballet) ──────────────────────
#
# dance and ballet now live as `--example`s in the shared `linkage-blaze-classic`
# crate. Example binaries land in target/<triple>/release/examples/<name>.

# Each example enables only its own `linkage-blaze-example-core` module, so unused
# example modules (and ballet's slow `MOTION` const) are never compiled.
_armatron_args       := _classic_args + " --features armatron"
_ballet_args         := _classic_args + " --features ballet"
_skeleton_clock_args := _classic_args + " --features skeleton-clock"
_clock_args          := _classic_args + " --features clock"

check-skeleton-clock-classic:
    cargo +esp check -p linkage-blaze-classic --example skeleton-clock {{_skeleton_clock_args}}

build-skeleton-clock-classic:
    source ~/export-esp.sh && cargo +esp build -p linkage-blaze-classic --example skeleton-clock {{_skeleton_clock_args}}

run-skeleton-clock-classic:
    just check-skeleton-clock-classic
    just build-skeleton-clock-classic
    source ~/export-esp.sh && cargo +esp run -p linkage-blaze-classic --example skeleton-clock {{_skeleton_clock_args}}

check-clock-classic:
    cargo +esp check -p linkage-blaze-classic --example clock {{_clock_args}}

build-clock-classic:
    source ~/export-esp.sh && cargo +esp build -p linkage-blaze-classic --example clock {{_clock_args}}

run-clock-classic:
    just check-clock-classic
    just build-clock-classic
    source ~/export-esp.sh && cargo +esp run -p linkage-blaze-classic --example clock {{_clock_args}}

check-armatron-classic:
    cargo +esp check -p linkage-blaze-classic --example armatron {{_armatron_args}}

build-armatron-classic:
    source ~/export-esp.sh && cargo +esp build -p linkage-blaze-classic --example armatron {{_armatron_args}}

run-armatron-classic:
    just check-armatron-classic
    just build-armatron-classic
    source ~/export-esp.sh && cargo +esp run -p linkage-blaze-classic --example armatron {{_armatron_args}}

check-ballet-classic:
    source ~/export-esp.sh && env RUSTFLAGS="{{_ballet_esp_rustflags}}" cargo +esp check -p linkage-blaze-classic --example ballet {{_ballet_args}}

build-ballet-classic:
    source ~/export-esp.sh && env RUSTFLAGS="{{_ballet_esp_rustflags}}" cargo +esp build -p linkage-blaze-classic --example ballet {{_ballet_args}}

size-ballet-classic:
    source ~/export-esp.sh && env RUSTFLAGS="{{_ballet_esp_rustflags}}" cargo +esp build -p linkage-blaze-classic --example ballet {{_ballet_args}}
    source ~/export-esp.sh && xtensa-esp32-elf-size target/xtensa-esp32-none-elf/release/examples/ballet
    source ~/export-esp.sh && xtensa-esp32-elf-size -A target/xtensa-esp32-none-elf/release/examples/ballet
    source ~/export-esp.sh && xtensa-esp32-elf-nm -S --size-sort target/xtensa-esp32-none-elf/release/examples/ballet | tail -n 30

run-ballet-classic:
    just check-ballet-classic
    just build-ballet-classic
    source ~/export-esp.sh && env RUSTFLAGS="{{_ballet_esp_rustflags}}" cargo +esp run -p linkage-blaze-classic --example ballet {{_ballet_args}}

# ── linkage-blaze-editor ──────────────────────────────────────────────────────

_editor_crate := "crates/linkage-blaze-editor"
_editor_www   := "crates/linkage-blaze-editor/www"

check-editor:
    cargo check -p linkage-blaze-editor --target wasm32-unknown-unknown

build-editor-deps:
    cd {{_editor_www}} && npm ci && npx esbuild deps-entry.js --bundle --format=esm --minify --outfile=vendor/editor-deps.js

build-editor:
    wasm-pack build {{_editor_crate}} --target web --out-dir www/pkg --out-name linkage_blaze_editor
