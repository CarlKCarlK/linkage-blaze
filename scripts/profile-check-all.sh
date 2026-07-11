#!/usr/bin/env bash
set -u

output_path="${1:-specs/check-all-timing.md}"
log_path="${output_path%.md}.log"
output_directory="$(dirname "$output_path")"
mkdir -p "$output_directory"

started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
started_epoch="$(date +%s)"
commit="$(git rev-parse --short HEAD 2>/dev/null || printf 'unknown')"
host="$(hostname 2>/dev/null || printf 'unknown')"
operating_system="$(uname -sr 2>/dev/null || printf 'unknown')"

labels=()
durations=()
statuses=()
commands=()

run_phase() {
    local label="$1"
    local command="$2"
    local phase_start phase_end duration status

    phase_start="$(date +%s)"
    printf '\n===== %s =====\n$ %s\n' "$label" "$command" | tee -a "$log_path"
    bash -lc "$command" 2>&1 | tee -a "$log_path"
    status="${PIPESTATUS[0]}"
    phase_end="$(date +%s)"
    duration=$((phase_end - phase_start))

    labels+=("$label")
    durations+=("$duration")
    statuses+=("$status")
    commands+=("$command")

    if (( status != 0 )); then
        printf '\nPhase failed: %s (exit %s)\n' "$label" "$status" >&2
        return "$status"
    fi
}

rm -f "$log_path"

run_phase "Generate board examples" \
    'cargo run --quiet -p linkage-blaze-xtask -- generate-board-examples' || exit $?
run_phase "Core tests" \
    'env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-core' || exit $?
run_phase "Core tests with alloc" \
    'env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-core --features alloc' || exit $?
run_phase "Core example integration tests" \
    'env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-core --features examples-armatron,examples-ballet,examples-clock,examples-skeleton-clock' || exit $?
run_phase "Utils tests" \
    'env RUSTFLAGS="-D warnings" cargo test -p linkage-blaze-utils' || exit $?
run_phase "Generate ESP examples and build" \
    'source ~/export-esp.sh && cargo run --quiet -p linkage-blaze-xtask -- build-esp-examples' || exit $?

for example in armatron ballet; do
    for board in 1 2 w 2w; do
        run_phase "RP ${example} ${board}" \
            "just --justfile crates/linkage-blaze-examples-rp/justfile build ${example} ${board}" || exit $?
    done
done
for board in w 2w; do
    run_phase "RP clock ${board}" \
        "just --justfile crates/linkage-blaze-examples-rp/justfile build clock ${board}" || exit $?
    run_phase "RP skeleton_clock ${board}" \
        "just --justfile crates/linkage-blaze-examples-rp/justfile build skeleton_clock ${board}" || exit $?
done

run_phase "Device Envoy RP example checks" \
    'cd ../mcu/device-envoy/crates/device-envoy-rp && cargo run --quiet --manifest-path xtask/Cargo.toml -- check-examples' || exit $?
run_phase "Utils WASM check" \
    'env RUSTFLAGS="-D warnings" cargo check -p linkage-blaze-utils --target wasm32-unknown-unknown' || exit $?
run_phase "Utils wasm-pack build" \
    'env RUSTFLAGS="-D warnings" wasm-pack build crates/linkage-blaze-utils --target web --out-dir www/pkg --out-name linkage_blaze_editor' || exit $?

finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
finished_epoch="$(date +%s)"
total_duration=$((finished_epoch - started_epoch))

{
    printf '# `check-all` timing profile\n\n'
    printf '<!-- todo0 consider deleting this profile once the check-all speed-up work is implemented and released. -->\n\n'
    printf 'Generated: `%s`  \n' "$finished_at"
    printf 'Started: `%s`  \n' "$started_at"
    printf 'Commit: `%s`  \n' "$commit"
    printf 'Host: `%s` (%s)\n\n' "$host" "$operating_system"
    printf 'This report measures one sequential run of the commands currently composing `just check-all`. Timings include command startup and compilation, use whole-second wall-clock resolution, and are affected by incremental build state, filesystem cache, CPU load, and network/cache availability.\n\n'
    printf 'The detailed command output is in [`%s`](../%s).\n\n' "$(basename "$log_path")" "$log_path"
    printf '## Summary\n\n'
    printf '| Phase | Wall time | Status |\n| --- | ---: | ---: |\n'
    for index in "${!labels[@]}"; do
        printf '| %s | %ss | %s |\n' "${labels[$index]}" "${durations[$index]}" "${statuses[$index]}"
    done
    printf '| **Total** | **%ss** | — |\n\n' "$total_duration"
    printf '## Profiling notes\n\n'
    printf -- '- The `test-core` and `test-utils` recipes are shown as their individual Cargo commands so their contributions are distinguishable.\n'
    printf -- '- Each RP board/example build is measured separately; repeated target builds may be incremental and therefore are not independent cold-build benchmarks.\n'
    printf -- '- This profile stops at the first failed phase, matching `check-all` failure behavior.\n'
    printf -- '- For optimization work, repeat this profile after each change and compare like-for-like build state and machine conditions.\n'
} > "$output_path"

printf 'Wrote %s\nWrote %s\n' "$output_path" "$log_path"
