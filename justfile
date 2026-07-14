set shell := ["bash", "-cu"]

run name chip="" port="":
    ./scripts/device-action.sh run "{{name}}" "{{chip}}" "{{port}}" "{{invocation_directory()}}"

check name chip="":
    ./scripts/device-action.sh check "{{name}}" "{{chip}}" "" "{{invocation_directory()}}"

build name chip="":
    ./scripts/device-action.sh build "{{name}}" "{{chip}}" "" "{{invocation_directory()}}"

_esp_args        := "--target xtensa-esp32-none-elf --release -Zbuild-std=core,alloc"
# RUSTFLAGS for ESP targets: -D warnings PLUS the linker script that .cargo/config.toml provides
# but that env RUSTFLAGS= would otherwise override.
_esp_rustflags   := "-D warnings -C link-arg=-Tlinkall.x"
_ballet_rustflags := "-D warnings -A long-running-const-eval"
_ballet_esp_rustflags := _esp_rustflags + " -A long-running-const-eval"

# Run the RP one-SPI armatron example. Board values are 1, 2, w, or 2w.
run-armatron-spi board="2":
    just --justfile crates/linkage-blaze-examples-rp/justfile run armatron_one_spi "{{board}}"

# ── Tests / checks ───────────────────────────────────────────────────────────

# Run linkage-blaze-core tests (unit tests + doc tests + alloc + examples integration tests)
test-core:
    env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-core
    env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-core --features alloc
    env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-core --features examples-armatron,examples-ballet,examples-clock,examples-skeleton-clock

# Run linkage-blaze-utils tests (editor render logic + the bvh-to-lb converter)
test-utils:
    env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-utils

# Check and build all crates
check-all:
    cargo run --quiet -p linkage-blaze-xtask -- generate-board-examples
    just test-core
    just test-utils
    source ~/export-esp.sh && cargo run --quiet -p linkage-blaze-xtask -- build-esp-examples
    just --justfile crates/linkage-blaze-examples-rp/justfile build armatron 1
    just --justfile crates/linkage-blaze-examples-rp/justfile build armatron 2
    just --justfile crates/linkage-blaze-examples-rp/justfile build armatron w
    just --justfile crates/linkage-blaze-examples-rp/justfile build armatron 2w
    just --justfile crates/linkage-blaze-examples-rp/justfile build ballet 1
    just --justfile crates/linkage-blaze-examples-rp/justfile build ballet 2
    just --justfile crates/linkage-blaze-examples-rp/justfile build ballet w
    just --justfile crates/linkage-blaze-examples-rp/justfile build ballet 2w
    just --justfile crates/linkage-blaze-examples-rp/justfile build clock w
    just --justfile crates/linkage-blaze-examples-rp/justfile build clock 2w
    just --justfile crates/linkage-blaze-examples-rp/justfile build skeleton_clock w
    just --justfile crates/linkage-blaze-examples-rp/justfile build skeleton_clock 2w
    cd ../mcu/device-envoy/crates/device-envoy-rp && cargo run --quiet --manifest-path xtask/Cargo.toml -- check-examples
    env RUSTFLAGS="-D warnings" cargo check -p linkage-blaze-utils --target wasm32-unknown-unknown
    env RUSTFLAGS="-D warnings" wasm-pack build crates/linkage-blaze-utils --target web --out-dir www/pkg --out-name linkage_blaze_editor

# Profile each command in check-all and write a Markdown report under specs/.
profile-check-all:
    ./scripts/profile-check-all.sh

# Alias for check-all
build-all:
    just check-all

# Build the static GitHub Pages artifact with immutable demo version URLs.
build-pages demo='':
    cargo run --quiet -p linkage-blaze-xtask -- build-pages "{{demo}}"

# Verify that generated CYD shell assets match the Device Envoy canonical source.
check-cyd-shell:
    just build-pages
    for page in target/pages/demos/*/*; do if test -f "$page/cyd-simulator.js"; then cmp ../mcu/device-envoy/crates/device-envoy-core/www/cyd-simulator.js "$page/cyd-simulator.js"; cmp ../mcu/device-envoy/crates/device-envoy-core/www/cyd-simulator.css "$page/cyd-simulator.css"; cmp ../mcu/device-envoy/crates/device-envoy-core/www/case.png "$page/case.png"; cmp ../mcu/device-envoy/crates/device-envoy-core/www/desk.jpg "$page/desk.jpg"; fi; done

# Build Pages and run the shared CYD browser contract tests.
test-cyd-browser:
    #!/usr/bin/env bash
    set -euo pipefail
    just build-pages
    server_port=$(python3 -c 'import socket; socket_ = socket.socket(); socket_.bind(("127.0.0.1", 0)); print(socket_.getsockname()[1]); socket_.close()')
    python3 scripts/cyd-test-server.py "$server_port" &
    server_process_id=$!
    trap 'kill "$server_process_id"' EXIT
    CYD_TEST_BASE_URL="http://127.0.0.1:$server_port" npx playwright test

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

# Regenerate the board/chip example matrix under `crates/linkage-blaze-examples-esp/examples/`.
generate-board-examples:
    cargo run --quiet -p linkage-blaze-xtask -- generate-board-examples

# ── linkage-blaze-examples-esp examples (dance, ballet) ───────────────────────────
#
# dance and ballet now live as generated `--example`s in the shared
# `linkage-blaze-examples-esp` crate. Example binaries land in
# target/<triple>/release/examples/<name>.

# Each example enables only its own `linkage-blaze-core` module, so unused
# example modules (and ballet's slow `MOTION` const) are never compiled.
_armatron_args       := _esp_args + " --features armatron"
_ballet_args         := _esp_args + " --features ballet"
_skeleton_clock_args := _esp_args + " --features skeleton-clock"
_clock_args          := _esp_args + " --features clock"

check-skeleton-clock-esp:
    just generate-board-examples
    cargo +esp check -p linkage-blaze-examples-esp --example skeleton_clock_esp32_generic {{_skeleton_clock_args}}

build-skeleton-clock-esp:
    just generate-board-examples
    source ~/export-esp.sh && cargo +esp build -p linkage-blaze-examples-esp --example skeleton_clock_esp32_generic {{_skeleton_clock_args}}

run-skeleton-clock-esp:
    just check-skeleton-clock-esp
    just build-skeleton-clock-esp
    source ~/export-esp.sh && cargo +esp run -p linkage-blaze-examples-esp --example skeleton_clock_esp32_generic {{_skeleton_clock_args}}

check-clock-esp:
    just generate-board-examples
    cargo +esp check -p linkage-blaze-examples-esp --example clock_esp32_generic {{_clock_args}}

build-clock-esp:
    just generate-board-examples
    source ~/export-esp.sh && cargo +esp build -p linkage-blaze-examples-esp --example clock_esp32_generic {{_clock_args}}

run-clock-esp:
    just check-clock-esp
    just build-clock-esp
    source ~/export-esp.sh && cargo +esp run -p linkage-blaze-examples-esp --example clock_esp32_generic {{_clock_args}}

check-armatron-esp:
    just generate-board-examples
    cargo +esp check -p linkage-blaze-examples-esp --example armatron_esp32_generic {{_armatron_args}}

build-armatron-esp:
    just generate-board-examples
    source ~/export-esp.sh && cargo +esp build -p linkage-blaze-examples-esp --example armatron_esp32_generic {{_armatron_args}}

run-armatron-esp:
    just check-armatron-esp
    just build-armatron-esp
    source ~/export-esp.sh && cargo +esp run -p linkage-blaze-examples-esp --example armatron_esp32_generic {{_armatron_args}}

check-ballet-esp:
    just generate-board-examples
    source ~/export-esp.sh && env RUSTFLAGS="{{_ballet_esp_rustflags}}" cargo +esp check -p linkage-blaze-examples-esp --example ballet_esp32_generic {{_ballet_args}}

build-ballet-esp:
    just generate-board-examples
    source ~/export-esp.sh && env RUSTFLAGS="{{_ballet_esp_rustflags}}" cargo +esp build -p linkage-blaze-examples-esp --example ballet_esp32_generic {{_ballet_args}}

size-ballet-esp:
    just generate-board-examples
    source ~/export-esp.sh && env RUSTFLAGS="{{_ballet_esp_rustflags}}" cargo +esp build -p linkage-blaze-examples-esp --example ballet_esp32_generic {{_ballet_args}}
    source ~/export-esp.sh && xtensa-esp32-elf-size target/xtensa-esp32-none-elf/release/examples/ballet_esp32_generic
    source ~/export-esp.sh && xtensa-esp32-elf-size -A target/xtensa-esp32-none-elf/release/examples/ballet_esp32_generic
    source ~/export-esp.sh && xtensa-esp32-elf-nm -S --size-sort target/xtensa-esp32-none-elf/release/examples/ballet_esp32_generic | tail -n 30

run-ballet-esp:
    just check-ballet-esp
    just build-ballet-esp
    source ~/export-esp.sh && env RUSTFLAGS="{{_ballet_esp_rustflags}}" cargo +esp run -p linkage-blaze-examples-esp --example ballet_esp32_generic {{_ballet_args}}

# ── linkage-blaze-utils ──────────────────────────────────────────────────────

_editor_crate := "crates/linkage-blaze-utils"
_editor_www   := "crates/linkage-blaze-utils/www"

check-editor:
    cargo check -p linkage-blaze-utils --target wasm32-unknown-unknown

build-editor-deps:
    cd {{_editor_www}} && npm ci && npx esbuild deps-entry.js --bundle --format=esm --minify --outfile=vendor/editor-deps.js

build-editor:
    wasm-pack build {{_editor_crate}} --target web --out-dir www/pkg --out-name linkage_blaze_editor
