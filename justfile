set shell := ["bash", "-cu"]

_classic_args := "--target xtensa-esp32-none-elf --release -Zbuild-std=core,alloc"
_c6_args      := "--target riscv32imac-unknown-none-elf --release --no-default-features --features esp32c6"

# ── Tests / checks ───────────────────────────────────────────────────────────

# Run linkage-blaze-core tests
test-core:
    cargo test -p linkage-blaze-core

# Build all embedded crates and run tests (build is required for real checking on microcontrollers)
check-all:
    just test-core
    source ~/export-esp.sh && just build-arm-classic
    just build-arm-c6
    source ~/export-esp.sh && just build-clock-classic

# Build everything
build:
    just test-core
    source ~/export-esp.sh && just build-arm-classic
    just build-arm-c6
    source ~/export-esp.sh && just build-clock-classic
    just build-arm-wasm
    just build-editor

# ── linkage-blaze-cyd ─────────────────────────────────────────────────────────

check-cyd:
    cargo +esp check -p linkage-blaze-cyd {{_classic_args}}

# ── linkage-blaze-armatron-classic ───────────────────────────────────────

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

