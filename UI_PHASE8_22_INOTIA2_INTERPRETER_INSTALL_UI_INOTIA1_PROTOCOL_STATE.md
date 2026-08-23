# Phase 8.22 — Inotia 2 Interpreter/Install UI + Inotia 1 Protocol State

## Inotia 1
- Remove Phase 8.17's forced command-0 branch.
- Prime the original cash protocol state at GOT offset `0x470` to `1` when the exact-title offline bridge starts.
- Trace the native protocol state at the command-1 response boundary.
- Retain the Phase 8.20 and Phase 8.21 obsolete-validator bypasses for now.

## Inotia 2
- Restore the 4,000-instruction execution slice after the 16,000-instruction Phase 8.21 experiment produced no field improvement.
- Suppress only the two installer progress-renderer calls; never bypass the required initializer.
- Optimize the ARM interpreter's page/memory hot path.
- Build release WASM with `codegen-units = 1` and `wasm-opt -O4`.
