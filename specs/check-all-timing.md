# `check-all` timing profile

<!-- todo0 consider deleting this profile once the check-all speed-up work is implemented and released. -->

Generated: `2026-07-11T19:48:45Z`  
Started: `2026-07-11T19:41:38Z`  
Commit: `0c3bbb9`  
Host: `carlk23` (Linux 6.6.114.1-microsoft-standard-WSL2)

This report measures one sequential run of the commands currently composing `just check-all`. Timings include command startup and compilation, use whole-second wall-clock resolution, and are affected by incremental build state, filesystem cache, CPU load, and network/cache availability.

The detailed command output is in [`check-all-timing.log`](../target/check-all-timing.log).

## Summary

| Phase | Wall time | Status |
| --- | ---: | ---: |
| Generate board examples | 0s | 0 |
| Core tests | 2s | 0 |
| Core tests with alloc | 2s | 0 |
| Core example integration tests | 28s | 0 |
| Utils tests | 1s | 0 |
| Generate ESP examples and build | 285s | 0 |
| RP armatron 1 | 2s | 0 |
| RP armatron 2 | 3s | 0 |
| RP armatron w | 5s | 0 |
| RP armatron 2w | 4s | 0 |
| RP ballet 1 | 8s | 0 |
| RP ballet 2 | 8s | 0 |
| RP ballet w | 7s | 0 |
| RP ballet 2w | 8s | 0 |
| RP clock w | 2s | 0 |
| RP skeleton_clock w | 3s | 0 |
| RP clock 2w | 2s | 0 |
| RP skeleton_clock 2w | 3s | 0 |
| Device Envoy RP example checks | 52s | 0 |
| Utils WASM check | 0s | 0 |
| Utils wasm-pack build | 2s | 0 |
| **Total** | **427s** | — |

## Profiling notes

- The `test-core` and `test-utils` recipes are shown as their individual Cargo commands so their contributions are distinguishable.
- Each RP board/example build is measured separately; repeated target builds may be incremental and therefore are not independent cold-build benchmarks.
- This profile stops at the first failed phase, matching `check-all` failure behavior.
- For optimization work, repeat this profile after each change and compare like-for-like build state and machine conditions.

## Unbounded parallelism experiment

An experimental version launched all generated ESP example Cargo commands immediately. In a fresh isolated target directory, it was still running after 418.5 seconds and was stopped; the earlier serial ESP phase completed in 285 seconds under different cache conditions. The output showed extensive contention on Cargo's package-cache, build-directory, and artifact-directory locks. This experiment does not justify unbounded fan-out; future work should group compatible examples into fewer Cargo invocations and use bounded target-level parallelism.

The implemented follow-up groups the 70 examples into 9 chip-target Cargo invocations, runs batches of 4 invocations, and gives each invocation 4 compiler jobs. A controlled fresh-target comparison measured 307.47 seconds grouped versus 447.88 seconds serial: 140.41 seconds faster, or 31.4%. Both runs used the same generated matrix, flags, and toolchain. The grouped approach is now the default ESP build strategy.
