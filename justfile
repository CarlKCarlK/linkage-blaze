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
    source ~/export-esp.sh && env RUSTFLAGS="{{_esp_rustflags}}" cargo +esp build -p linkage-blaze-clock-classic {{_classic_args}}
    env RUSTFLAGS="-D warnings" wasm-pack build crates/linkage-blaze-armatron-wasm --target web --out-dir www/pkg --out-name linkage_blaze_armatron_wasm
    env RUSTFLAGS="-D warnings" wasm-pack build crates/linkage-blaze-editor --target web --out-dir www/pkg --out-name linkage_blaze_editor
    env RUSTFLAGS="-D warnings" wasm-pack build crates/linkage-blaze-printer-wasm --target web --out-dir web/pkg --out-name linkage_blaze_printer_wasm
    env RUSTFLAGS="-D warnings" wasm-pack build crates/linkage-blaze-mocap-wasm --target web --out-dir web/pkg --out-name linkage_blaze_mocap_wasm

# Alias for check-all
build:
    just check-all

# Generate docs and open in browser
docs:
    env RUSTFLAGS="-D warnings" cargo doc -p linkage-blaze-core --no-deps --features alloc --open

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

# ── linkage-blaze-clock-classic ─────────────────────────────────────────

check-clock-classic:
    cargo +esp check -p linkage-blaze-clock-classic {{_classic_args}}

build-clock-classic:
    source ~/export-esp.sh && cargo +esp build -p linkage-blaze-clock-classic {{_classic_args}}

run-clock-classic:
    just check-clock-classic
    just build-clock-classic
    source ~/export-esp.sh && cargo +esp run -p linkage-blaze-clock-classic {{_classic_args}}

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
