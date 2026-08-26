# WIPI Player Phase 8.42 — Quiet Inotia1 Database Diagnostics

Phase 8.42 is a performance-cleanup build based directly on Phase 8.40. It does not include the Phase 8.41 Continue-position rescue hook. The user's recovered save now loads under Phase 8.40, so the rescue hook is no longer required for routine gameplay.

## Why this phase exists

A field log from the recovered save proved that Phase 8.40 itself still emitted thousands of legacy Inotia1 database diagnostics. These traces predate Phase 8.41 and therefore remained after downgrading. They include INFO logging for nearly every `char.dat`, `map.dat`, `tile.dat`, `mon.dat`, and `pattern.dat` open/read/seek/select, including reads executed on a timer thread. At later game progress this creates substantial formatting and console-forwarding overhead.

## Phase 8.42 changes

- Demotes the per-open Phase7_21 'fix active' message to DEBUG.
- Demotes generic Inotia1 database OPEN/SEEK/SELECT diagnostics to DEBUG.
- Completely removes expensive fingerprint/head/tail formatting for non-save Inotia1 READ operations from the INFO path.
- Removes the obsolete Phase7_19 save0 CPU/frame/code/stack snapshot.
- Retains concise INFO fingerprints only for `save0.dat`/`save1.dat` reads and writes so future resurrection/save debugging remains possible without flooding gameplay.
- Keeps Phase 8.40 emergency-prayer purchase behavior and the exact resurrection-context repair unchanged.
- Keeps the Phase 8.39 exception-only ARM fault trace.
- Keeps the Phase 8.37 12-item cash catalog, main-name repair, and performance scheduling unchanged.
- Inotia2 is unchanged.

## Test

1. Install TestFlight 0.1.42 and confirm `PHASE8_42_RUNTIME_SENTINEL`.
2. Use the newly repaired working save.
3. Walk around the same field where Phase 8.40 felt laggy.
4. Export a short log after 30–60 seconds of normal movement. The log should no longer contain hundreds/thousands of `INOTIA1_SAVE` resource-database lines.
5. Do not perform the resurrection experiments until normal movement performance is confirmed.
