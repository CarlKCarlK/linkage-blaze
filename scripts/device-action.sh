#!/usr/bin/env bash
set -euo pipefail

action="${1:-}"
name="${2:-}"
chip="${3:-}"
port_arg="${4:-}"
invocation_dir="${5:-}"

if [[ "$name" == *.rs ]]; then
  name="${name%.rs}"
fi
name="${name//-/_}"

if [[ -z "$port_arg" ]]; then
  case "$chip" in
    /dev/*|tty*|USB*|ACM*)
      port_arg="$chip"
      chip=""
      ;;
  esac
fi

port="${port_arg:-${ESPFLASH_PORT:-}}"

normalize_port() {
  local port_value="${1:-}"
  if [[ -z "$port_value" ]]; then
    printf "%s" ""
    return
  fi
  if [[ "$port_value" == /dev/* ]]; then
    printf "%s" "$port_value"
    return
  fi
  if [[ "$port_value" == tty* ]]; then
    printf "%s" "/dev/$port_value"
    return
  fi
  if [[ "$port_value" == ACM* || "$port_value" == USB* ]]; then
    printf "%s" "/dev/tty$port_value"
    return
  fi
  printf "%s" "$port_value"
}

list_serial_ports() {
  shopt -s nullglob
  local serial_port=""
  for serial_port in /dev/ttyUSB* /dev/ttyACM*; do
    if [[ -e "$serial_port" ]]; then
      printf "%s\n" "$serial_port"
    fi
  done
}

port="$(normalize_port "$port")"

infer_board_example_from_invocation_dir() {
  if [[ -z "$invocation_dir" ]]; then
    return
  fi

  local relative_invocation_dir="$invocation_dir"
  local workspace_root="$PWD"
  if [[ "$relative_invocation_dir" == "$workspace_root"* ]]; then
    relative_invocation_dir="${relative_invocation_dir#"$workspace_root"/}"
  fi
  if [[ "$relative_invocation_dir" != crates/linkage-blaze-esp/examples/*/* ]]; then
    return
  fi

  local relative_after_examples="${relative_invocation_dir#crates/linkage-blaze-esp/examples/}"
  local chip_dir="${relative_after_examples%%/*}"
  local board_dir="${relative_after_examples#${chip_dir}/}"
  board_dir="${board_dir%%/*}"

  local chip_feature=""
  local inferred_chip=""
  local target=""
  case "$chip_dir" in
    esp32)
      chip_feature="esp32"
      inferred_chip="esp32"
      target="xtensa-esp32-none-elf"
      ;;
    c2)
      chip_feature="esp32c2"
      inferred_chip="c2"
      target="riscv32imc-unknown-none-elf"
      ;;
    c3)
      chip_feature="esp32c3"
      inferred_chip="c3"
      target="riscv32imc-unknown-none-elf"
      ;;
    c5)
      chip_feature="esp32c5"
      inferred_chip="c5"
      target="riscv32imac-unknown-none-elf"
      ;;
    c6)
      chip_feature="esp32c6"
      inferred_chip="c6"
      target="riscv32imac-unknown-none-elf"
      ;;
    c61)
      chip_feature="esp32c61"
      inferred_chip="c61"
      target="riscv32imac-unknown-none-elf"
      ;;
    h2)
      chip_feature="esp32h2"
      inferred_chip="h2"
      target="riscv32imac-unknown-none-elf"
      ;;
    s2)
      chip_feature="esp32s2"
      inferred_chip="s2"
      target="xtensa-esp32s2-none-elf"
      ;;
    s3)
      chip_feature="esp32s3"
      inferred_chip="s3"
      target="xtensa-esp32s3-none-elf"
      ;;
    *)
      return
      ;;
  esac

  if [[ ! -f "$invocation_dir/${name}.rs" ]]; then
    return
  fi

  local inferred_example="${name}_${chip_feature}_${board_dir}"
  if grep -Eq "^[[:space:]]*name[[:space:]]*=[[:space:]]*\"${inferred_example}\"[[:space:]]*$" crates/linkage-blaze-esp/Cargo.toml; then
    name="$inferred_example"
    if [[ -z "$chip" ]]; then
      chip="$inferred_chip"
    fi
  fi
}

if [[ -z "$action" || -z "$name" ]]; then
  echo "usage: scripts/device-action.sh <run|check|build> <name> [chip] [port] [invocation_dir]" >&2
  echo "port can be /dev/ttyUSB0, ttyUSB0, USB0, ACM0, etc." >&2
  exit 1
fi

infer_board_example_from_invocation_dir

if [[ -z "$chip" ]]; then
  case "$name" in
    *_esp32_*)
      chip="esp32"
      ;;
    *_esp32c2_*)
      chip="c2"
      ;;
    *_esp32c3_*)
      chip="c3"
      ;;
    *_esp32c5_*)
      chip="c5"
      ;;
    *_esp32c6_*)
      chip="c6"
      ;;
    *_esp32c61_*)
      chip="c61"
      ;;
    *_esp32h2_*)
      chip="h2"
      ;;
    *_esp32s2_*)
      chip="s2"
      ;;
    *_esp32s3_*)
      chip="s3"
      ;;
  esac
fi

has_example=0
if grep -Eq "^[[:space:]]*name[[:space:]]*=[[:space:]]*\"${name}\"[[:space:]]*$" crates/linkage-blaze-esp/Cargo.toml; then
  has_example=1
fi

if [[ "$has_example" -eq 0 ]]; then
  echo "unknown example '$name' (no matching example in crates/linkage-blaze-esp/Cargo.toml)" >&2
  exit 1
fi

cargo_bin=(cargo)
build_std_args=()
target=""
chip_feature=""
example_feature=""
extra_env=()

case "$chip" in
  c6)
    target="riscv32imac-unknown-none-elf"
    chip_feature="esp32c6"
    ;;
  c2)
    target="riscv32imc-unknown-none-elf"
    chip_feature="esp32c2"
    ;;
  c3)
    target="riscv32imc-unknown-none-elf"
    chip_feature="esp32c3"
    ;;
  c5)
    target="riscv32imac-unknown-none-elf"
    chip_feature="esp32c5"
    ;;
  h2)
    target="riscv32imac-unknown-none-elf"
    chip_feature="esp32h2"
    ;;
  c61)
    target="riscv32imac-unknown-none-elf"
    chip_feature="esp32c61"
    ;;
  esp32)
    target="xtensa-esp32-none-elf"
    chip_feature="esp32"
    cargo_bin=(cargo +esp)
    build_std_args=(-Zbuild-std=core,alloc)
    ;;
  s2)
    target="xtensa-esp32s2-none-elf"
    chip_feature="esp32s2"
    cargo_bin=(cargo +esp)
    build_std_args=(-Zbuild-std=core,alloc)
    ;;
  s3)
    target="xtensa-esp32s3-none-elf"
    chip_feature="esp32s3"
    cargo_bin=(cargo +esp)
    build_std_args=(-Zbuild-std=core,alloc)
    ;;
  *)
    echo "invalid chip '$chip' (expected one of: c6, c5, c61, c2, c3, h2, esp32, s2, s3)" >&2
    exit 1
    ;;
esac

case "$name" in
  armatron_*)
    example_feature="armatron"
    ;;
  ballet_*)
    example_feature="ballet"
    extra_env=(env RUSTFLAGS="-D warnings -C link-arg=-Tlinkall.x -A long-running-const-eval")
    ;;
  clock_*)
    example_feature="clock"
    ;;
  skeleton_clock_*)
    example_feature="skeleton-clock"
    ;;
  *)
    echo "unknown linkage-blaze example family for '$name'" >&2
    exit 1
    ;;
esac

if [[ "${#build_std_args[@]}" -gt 0 ]]; then
  source "$HOME/export-esp.sh"
fi

if [[ "$action" == "run" && -z "$port" ]]; then
  mapfile -t detected_ports < <(list_serial_ports)
  if [[ "${#detected_ports[@]}" -gt 1 ]]; then
    echo "multiple serial devices detected; refusing to auto-select a flash port:" >&2
    printf "  - %s\n" "${detected_ports[@]}" >&2
    echo "pass a port explicitly, for example:" >&2
    echo "  just run $name $chip ttyUSB1" >&2
    exit 1
  fi
  if [[ "${#detected_ports[@]}" -eq 1 ]]; then
    port="${detected_ports[0]}"
  fi
fi

if [[ "$action" == "run" && -n "$port" ]]; then
  export ESPFLASH_PORT="$port"
fi

command_prefix=()
if [[ "${#extra_env[@]}" -gt 0 ]]; then
  command_prefix=("${extra_env[@]}")
fi

"${command_prefix[@]}" "${cargo_bin[@]}" "$action" \
  -p linkage-blaze-esp \
  --example "$name" \
  --target "$target" \
  --release \
  --no-default-features \
  --features "${example_feature},${chip_feature}" \
  "${build_std_args[@]}"
