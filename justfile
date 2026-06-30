set shell := ["bash", "-cu"]

_classic_args    := "--target xtensa-esp32-none-elf --release -Zbuild-std=core,alloc"
_c6_args         := "--target riscv32imac-unknown-none-elf --release --no-default-features --features esp32c6"
# RUSTFLAGS for ESP targets: -D warnings PLUS the linker script that .cargo/config.toml provides
# but that env RUSTFLAGS= would otherwise override.
_esp_rustflags   := "-D warnings -C link-arg=-Tlinkall.x"

# ── Tests / checks ───────────────────────────────────────────────────────────

# Run linkage-blaze-core tests (unit tests + doc tests + alloc integration tests)
test-core:
    env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-core
    env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-core --features alloc

# Check and build all crates
check-all:
    env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-core
    env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-core --features alloc
    source ~/export-esp.sh && env RUSTFLAGS="{{_esp_rustflags}}" cargo +esp check -p linkage-blaze-cyd {{_classic_args}}
    source ~/export-esp.sh && env RUSTFLAGS="{{_esp_rustflags}}" cargo +esp build -p linkage-blaze-armatron-classic {{_classic_args}}
    env RUSTFLAGS="{{_esp_rustflags}}" cargo build -p linkage-blaze-armatron-c6 {{_c6_args}}
    source ~/export-esp.sh && env RUSTFLAGS="{{_esp_rustflags}}" cargo +esp build -p linkage-blaze-classic --example clock {{_clock_args}}
    source ~/export-esp.sh && env RUSTFLAGS="{{_esp_rustflags}}" cargo +esp build -p linkage-blaze-classic --example skeleton-clock {{_skeleton_clock_args}}
    source ~/export-esp.sh && env RUSTFLAGS="{{_esp_rustflags}}" cargo +esp build -p linkage-blaze-classic --example ballet {{_ballet_args}}
    env RUSTFLAGS="-D warnings" wasm-pack build crates/linkage-blaze-classic-wasm --target web --out-dir www/pkg --out-name linkage_blaze_classic_wasm
    env RUSTFLAGS="-D warnings" wasm-pack build crates/linkage-blaze-skeleton-clock-wasm --target web --out-dir www/pkg --out-name linkage_blaze_skeleton_clock_wasm
    env RUSTFLAGS="-D warnings" wasm-pack build crates/linkage-blaze-armatron-wasm --target web --out-dir www/pkg --out-name linkage_blaze_armatron_wasm
    env RUSTFLAGS="-D warnings" wasm-pack build crates/linkage-blaze-editor --target web --out-dir www/pkg --out-name linkage_blaze_editor
    env RUSTFLAGS="-D warnings" wasm-pack build crates/linkage-blaze-printer-wasm --target web --out-dir web/pkg --out-name linkage_blaze_printer_wasm
    env RUSTFLAGS="-D warnings" wasm-pack build crates/linkage-blaze-mocap-wasm --target web --out-dir web/pkg --out-name linkage_blaze_mocap_wasm

# Alias for check-all
build:
    just check-all

# Build the static GitHub Pages artifact with immutable demo version URLs.
build-pages demo='':
    bash .tools/build_pages.sh "{{demo}}"

# Freeze the current live web assets for one demo into a new immutable Pages version.
bump-demo-version demo version='':
    bash .tools/bump_demo_version.sh "{{demo}}" "{{version}}"

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
    env RUSTFLAGS="-D warnings" cargo doc -p linkage-blaze-mocap --no-deps
    env RUSTFLAGS="-D warnings" cargo doc -p linkage-blaze-armatron-core --no-deps
    env RUSTFLAGS="-D warnings" cargo doc -p linkage-blaze-printer-wasm --no-deps

    cp -R target/doc/linkage_blaze_core "$rustdoc_dir/"
    cp -R target/doc/linkage_blaze_mocap "$rustdoc_dir/"
    cp -R target/doc/linkage_blaze_armatron_core "$rustdoc_dir/"
    cp -R target/doc/linkage_blaze_printer_wasm "$rustdoc_dir/"
    cp target/doc/crates.js target/doc/help.html target/doc/search-index.js target/doc/settings.html target/doc/src-files.js "$rustdoc_dir/" 2>/dev/null || true

    {
        printf -- '# linkage-blaze AI docs bundle\n\n'
        printf -- 'Generated: %s UTC\n\n' "$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
        printf -- 'This bundle is intended for an outside AI reviewer. It includes repository guidance, Markdown docs, Cargo manifests, and generated rustdoc HTML copied under `rustdoc/`.\n\n'
        printf -- '## Rustdoc entry points\n\n'
        printf -- '%s\n' '- `rustdoc/linkage_blaze_core/index.html`'
        printf -- '%s\n' '- `rustdoc/linkage_blaze_mocap/index.html`'
        printf -- '%s\n' '- `rustdoc/linkage_blaze_armatron_core/index.html`'
        printf -- '%s\n\n' '- `rustdoc/linkage_blaze_printer_wasm/index.html`'
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

# ── linkage-blaze-cyd ─────────────────────────────────────────────────────────

check-cyd:
    cargo +esp check -p linkage-blaze-cyd {{_classic_args}}

# ── linkage-blaze-armatron-classic ───────────────────────────────────────

_arm_classic_elf := "target/xtensa-esp32-none-elf/release/linkage-blaze-armatron-classic"

# Build and report flash + RAM usage for the ESP32 classic target
size-arm-classic:
    just build-arm-classic
    source ~/export-esp.sh && python3 .tools/elf_size.py {{_arm_classic_elf}}

check-arm-classic:
    cargo +esp check -p linkage-blaze-armatron-classic {{_classic_args}}

build-arm-classic:
    source ~/export-esp.sh && cargo +esp build -p linkage-blaze-armatron-classic {{_classic_args}}

run-arm-classic:
    just check-arm-classic
    just build-arm-classic
    source ~/export-esp.sh && cargo +esp run -p linkage-blaze-armatron-classic {{_classic_args}}

# ── linkage-blaze-armatron-c6 ───────────────────────────────────────────

check-arm-c6:
    cargo check -p linkage-blaze-armatron-c6 {{_c6_args}}

build-arm-c6:
    cargo build -p linkage-blaze-armatron-c6 {{_c6_args}}

run-arm-c6:
    just check-arm-c6
    just build-arm-c6
    cargo run -p linkage-blaze-armatron-c6 {{_c6_args}}

# ── linkage-blaze-classic examples (dance, ballet) ──────────────────────
#
# dance and ballet now live as `--example`s in the shared `linkage-blaze-classic`
# crate. Example binaries land in target/<triple>/release/examples/<name>.

# Each example enables only its own `linkage-blaze-example-core` module, so unused
# example modules (and ballet's slow `MOTION` const) are never compiled.
_ballet_args         := _classic_args + " --features ballet"
_skeleton_clock_args := _classic_args + " --features skeleton-clock"
_clock_args          := _classic_args + " --features clock"

generate-skeleton-clock:
    cargo run -p linkage-blaze-mocap --example specialize_dance

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

# ── linkage-blaze-classic-wasm (browser-simulated CYD `ballet`) ─────────────
_ballet_wasm_crate := "crates/linkage-blaze-classic-wasm"
_ballet_wasm_www   := "crates/linkage-blaze-classic-wasm/www"
_ballet_wasm_port  := "8085"

check-ballet-wasm:
    cargo check -p linkage-blaze-classic-wasm --target wasm32-unknown-unknown

build-ballet-wasm:
    wasm-pack build {{_ballet_wasm_crate}} --target web --out-dir www/pkg --out-name linkage_blaze_classic_wasm

serve-ballet-wasm port=_ballet_wasm_port:
    -lsof -ti:{{port}} | xargs -r kill
    cd {{_ballet_wasm_www}} && python3 ../../../.tools/no_cache_http_server.py {{port}}

run-ballet-wasm port=_ballet_wasm_port:
    just check-ballet-wasm
    just build-ballet-wasm
    just serve-ballet-wasm {{port}}

# ── linkage-blaze-skeleton-clock-wasm (browser-simulated CYD `skeleton_clock`) ─
_skeleton_clock_wasm_crate := "crates/linkage-blaze-skeleton-clock-wasm"
_skeleton_clock_wasm_www   := "crates/linkage-blaze-skeleton-clock-wasm/www"
_skeleton_clock_wasm_port  := "8086"

check-skeleton-clock-wasm:
    cargo check -p linkage-blaze-skeleton-clock-wasm --target wasm32-unknown-unknown

build-skeleton-clock-wasm:
    wasm-pack build {{_skeleton_clock_wasm_crate}} --target web --out-dir www/pkg --out-name linkage_blaze_skeleton_clock_wasm

serve-skeleton-clock-wasm port=_skeleton_clock_wasm_port:
    -lsof -ti:{{port}} | xargs -r kill
    cd {{_skeleton_clock_wasm_www}} && python3 ../../../.tools/no_cache_http_server.py {{port}}

run-skeleton-clock-wasm port=_skeleton_clock_wasm_port:
    just check-skeleton-clock-wasm
    just build-skeleton-clock-wasm
    just serve-skeleton-clock-wasm {{port}}

generate-ballet:
    cargo run -p linkage-blaze-mocap --example generate_ballet

check-ballet-classic:
    cargo +esp check -p linkage-blaze-classic --example ballet {{_ballet_args}}

build-ballet-classic:
    source ~/export-esp.sh && cargo +esp build -p linkage-blaze-classic --example ballet {{_ballet_args}}

size-ballet-classic:
    source ~/export-esp.sh && cargo +esp build -p linkage-blaze-classic --example ballet {{_ballet_args}}
    source ~/export-esp.sh && xtensa-esp32-elf-size target/xtensa-esp32-none-elf/release/examples/ballet
    source ~/export-esp.sh && xtensa-esp32-elf-size -A target/xtensa-esp32-none-elf/release/examples/ballet
    source ~/export-esp.sh && xtensa-esp32-elf-nm -S --size-sort target/xtensa-esp32-none-elf/release/examples/ballet | tail -n 30

run-ballet-classic:
    just check-ballet-classic
    just build-ballet-classic
    source ~/export-esp.sh && cargo +esp run -p linkage-blaze-classic --example ballet {{_ballet_args}}

# ── linkage-blaze-armatron-wasm (web simulator + 3D viewer) ─────────────────

_arm_wasm_crate      := "crates/linkage-blaze-armatron-wasm"
_arm_wasm_www        := "crates/linkage-blaze-armatron-wasm/www"
_arm_wasm_viewer_www := "crates/linkage-blaze-armatron-wasm/www/viewer"
_arm_sim_port        := "8081"
_arm_viewer_port     := "8080"

check-arm-wasm:
    cargo check -p linkage-blaze-armatron-wasm --target wasm32-unknown-unknown

build-arm-wasm:
    wasm-pack build {{_arm_wasm_crate}} --target web --out-dir www/pkg --out-name linkage_blaze_armatron_wasm

serve-arm-wasm port=_arm_sim_port:
    -lsof -ti:{{port}} | xargs -r kill
    cd {{_arm_wasm_www}} && python3 ../../../.tools/no_cache_http_server.py {{port}}

run-arm-wasm port=_arm_sim_port:
    just check-arm-wasm
    just build-arm-wasm
    just serve-arm-wasm {{port}}

serve-arm-viewer-wasm port=_arm_viewer_port:
    -lsof -ti:{{port}} | xargs -r kill
    cd {{_arm_wasm_viewer_www}} && python3 ../../../../.tools/no_cache_http_server.py {{port}}

run-arm-viewer-wasm port=_arm_viewer_port:
    just check-arm-wasm
    just build-arm-wasm
    just serve-arm-viewer-wasm {{port}}

# ── linkage-blaze-editor ──────────────────────────────────────────────────────

_editor_crate := "crates/linkage-blaze-editor"
_editor_www   := "crates/linkage-blaze-editor/www"
_editor_port  := "8082"

check-editor:
    cargo check -p linkage-blaze-editor --target wasm32-unknown-unknown

build-editor:
    wasm-pack build {{_editor_crate}} --target web --out-dir www/pkg --out-name linkage_blaze_editor

serve-editor port=_editor_port:
    cd {{_editor_www}} && python3 ../../../.tools/no_cache_http_server.py {{port}}

run-editor port=_editor_port:
    just check-editor
    just build-editor
    just serve-editor {{port}}

# ── linkage-blaze-printer-wasm ────────────────────────────────────────────────

_printer_crate := "crates/linkage-blaze-printer-wasm"
_printer_www   := "crates/linkage-blaze-printer-wasm/web"
_printer_port  := "8083"

check-printer-wasm:
    cargo check -p linkage-blaze-printer-wasm --target wasm32-unknown-unknown

build-printer-wasm:
    wasm-pack build {{_printer_crate}} --target web --out-dir web/pkg --out-name linkage_blaze_printer_wasm

serve-printer-wasm port=_printer_port:
    -lsof -ti:{{port}} | xargs -r kill
    cd {{_printer_www}} && python3 ../../../.tools/no_cache_http_server.py {{port}}

run-printer-wasm port=_printer_port:
    just check-printer-wasm
    just build-printer-wasm
    just serve-printer-wasm {{port}}

# ── linkage-blaze-mocap-wasm ─────────────────────────────────────────────────

_mocap_wasm_crate := "crates/linkage-blaze-mocap-wasm"
_mocap_wasm_www   := "crates/linkage-blaze-mocap-wasm/web"
_mocap_wasm_port  := "8084"

check-mocap-wasm:
    cargo check -p linkage-blaze-mocap-wasm --target wasm32-unknown-unknown

build-mocap-wasm:
    wasm-pack build {{_mocap_wasm_crate}} --target web --out-dir web/pkg --out-name linkage_blaze_mocap_wasm

serve-mocap-wasm port=_mocap_wasm_port:
    -lsof -ti:{{port}} | xargs -r kill
    cd {{_mocap_wasm_www}} && python3 ../../../.tools/no_cache_http_server.py {{port}}

run-mocap-bvh port=_mocap_wasm_port:
    @echo "Open http://127.0.0.1:{{port}}/ and click Load sample."
    just check-mocap-wasm
    just build-mocap-wasm
    just serve-mocap-wasm {{port}}
