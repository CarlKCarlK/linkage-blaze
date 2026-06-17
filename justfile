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
    source ~/export-esp.sh && just build-armatron-classic
    just build-armatron-c6
    source ~/export-esp.sh && just build-clock-classic

# ── linkage-blaze-cyd ─────────────────────────────────────────────────────────

check-cyd:
    cargo +esp check -p linkage-blaze-cyd {{_classic_args}}

# ── linkage-blaze-armatron-classic ───────────────────────────────────────

check-armatron-classic:
    cargo +esp check -p linkage-blaze-armatron-classic {{_classic_args}}

build-armatron-classic:
    source ~/export-esp.sh && cargo +esp build -p linkage-blaze-armatron-classic {{_classic_args}}

run-armatron-classic:
    source ~/export-esp.sh && cargo +esp run -p linkage-blaze-armatron-classic {{_classic_args}}

# ── linkage-blaze-armatron-c6 ───────────────────────────────────────────

check-armatron-c6:
    cargo check -p linkage-blaze-armatron-c6 {{_c6_args}}

build-armatron-c6:
    cargo build -p linkage-blaze-armatron-c6 {{_c6_args}}

run-armatron-c6:
    cargo run -p linkage-blaze-armatron-c6 {{_c6_args}}

# ── linkage-blaze-clock-classic ─────────────────────────────────────────

check-clock-classic:
    cargo +esp check -p linkage-blaze-clock-classic {{_classic_args}}

build-clock-classic:
    source ~/export-esp.sh && cargo +esp build -p linkage-blaze-clock-classic {{_classic_args}}

run-clock-classic:
    source ~/export-esp.sh && cargo +esp run -p linkage-blaze-clock-classic {{_classic_args}}

# ── linkage-blaze-armatron-sim (web simulator) ───────────────────────────────

_armatron_core_crate := "crates/linkage-blaze-armatron-core"
_armatron_core_www   := "crates/linkage-blaze-armatron-core/www"
_armatron_sim_port  := "8081"

build-armatron-sim:
    wasm-pack build {{_armatron_core_crate}} --target web --out-dir www/pkg --out-name linkage_blaze_armatron_core

serve-armatron-sim port=_armatron_sim_port:
    cd {{_armatron_core_www}} && python3 ../../../.tools/no_cache_http_server.py {{port}}

run-armatron-sim port=_armatron_sim_port:
    just build-armatron-sim
    just serve-armatron-sim {{port}}

# ── linkage-blaze-armatron-wasm (3D viewer) ───────────────────────────────────

_armatron_wasm_crate := "crates/linkage-blaze-armatron-wasm"
_armatron_wasm_www   := "crates/linkage-blaze-armatron-wasm/www"
_armatron_wasm_port  := "8080"

build-armatron-wasm:
    wasm-pack build {{_armatron_wasm_crate}} --target web --out-dir www/pkg --out-name linkage_blaze_armatron_wasm

serve-armatron-wasm port=_armatron_wasm_port:
    cd {{_armatron_wasm_www}} && python3 -m http.server {{port}}

run-armatron-wasm port=_armatron_wasm_port:
    just build-armatron-wasm
    just serve-armatron-wasm {{port}}

# ── linkage-blaze-editor ──────────────────────────────────────────────────────

_editor_crate := "crates/linkage-blaze-editor"
_editor_www   := "crates/linkage-blaze-editor/www"
_editor_port  := "8082"

build-editor:
    wasm-pack build {{_editor_crate}} --target web --out-dir www/pkg --out-name linkage_blaze_editor

serve-editor port=_editor_port:
    cd {{_editor_www}} && python3 ../../../.tools/no_cache_http_server.py {{port}}

run-editor port=_editor_port:
    just build-editor
    just serve-editor {{port}}
