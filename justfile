set shell := ["bash", "-cu"]

_cyd_esp32_args := "-p cyd-esp32 --target xtensa-esp32-none-elf --release -Zbuild-std=core,alloc"
_cyd_args := "-p robot-arm-cyd --target xtensa-esp32-none-elf --release -Zbuild-std=core,alloc"
_cyd_clock_args := "-p robot-arm-cyd-clock --target xtensa-esp32-none-elf --release -Zbuild-std=core,alloc"
_cyd_sim_crate := "crates/robot-arm-cyd-sim"
_cyd_sim_www := "crates/robot-arm-cyd-sim/www"
_cyd_sim_port := "8081"
_wasm_crate := "crates/robot-arm-wasm"
_wasm_www := "crates/robot-arm-wasm/www"
_wasm_port := "8080"

# Check shared CYD ESP32 hardware crate
check-cyd-esp32:
    cargo +esp check {{_cyd_esp32_args}}

# Check robot-arm-cyd (ESP32)
check-cyd:
    cargo +esp check {{_cyd_args}}

# Build robot-arm-cyd (ESP32)
build-cyd:
    source ~/export-esp.sh && cargo +esp build {{_cyd_args}}

# Flash and monitor robot-arm-cyd on the CYD board
run-cyd:
    source ~/export-esp.sh && cargo +esp run {{_cyd_args}}

# Check robot-arm-cyd-clock (ESP32)
check-cyd-clock:
    cargo +esp check {{_cyd_clock_args}}

# Build robot-arm-cyd-clock (ESP32)
build-cyd-clock:
    source ~/export-esp.sh && cargo +esp build {{_cyd_clock_args}}

# Flash and monitor robot-arm-cyd-clock on the CYD board
run-cyd-clock:
    source ~/export-esp.sh && cargo +esp run {{_cyd_clock_args}}

# Flash and monitor robot-arm-c6 on the ESP32-C6 board
run-c6:
    cargo run -p robot-arm-c6 --target riscv32imac-unknown-none-elf --release --no-default-features --features esp32c6

# Build the CYD simulator WASM bundle into www/pkg
build-cyd-sim:
    wasm-pack build {{_cyd_sim_crate}} --target web --out-dir www/pkg --out-name robot_arm_cyd_sim_v3

# Serve the CYD simulator web app
serve-cyd-sim port=_cyd_sim_port:
    cd {{_cyd_sim_www}} && python3 ../../../.tools/no_cache_http_server.py {{port}}

# Build and serve the CYD simulator
run-cyd-sim port=_cyd_sim_port:
    just build-cyd-sim
    just serve-cyd-sim {{port}}

# Build the robot-arm-wasm bundle into www/pkg
build-wasm-ui:
    wasm-pack build {{_wasm_crate}} --target web --out-dir www/pkg

# Serve the robot-arm-wasm web app
serve-wasm-ui port=_wasm_port:
    cd {{_wasm_www}} && python3 -m http.server {{port}}

# Build and serve the robot-arm-wasm web app
run-wasm-ui port=_wasm_port:
    just build-wasm-ui
    just serve-wasm-ui {{port}}
