# WIPI Player Phase 8.19 — Inotia 2 installed-state/performance + Inotia 1 rejection trace

Phase 8.19 is a full-repository release built on the stable Phase 8.18 baseline.

## Inotia 2 (010100D5 / PD007974)

Phase 8.18 remains the compatibility baseline: the native installer/initializer is allowed to run because field testing proved that skipping it causes `메모리에러`. The buffered install write-back and 4,000-instruction execution slice remain.

Phase 8.19 makes three conservative changes based on the Phase 8.18 field log:

1. **Preserve the real installed +8-byte cache footer.** After the installer closes each of the four generated caches, the title deterministically appends two four-byte values. Previous repair code stripped this valid footer because it compared only against the packaged `p/` length. Phase 8.19 accepts only `base` or `base + 8` for these exact cache files, while still repairing the old multi-copy corruption. This may also let the game's own installed-state check succeed on the *next* launch instead of entering the installer again.
2. **Shared per-launch resource cache.** `i_pack.dat`, `eventdata.dat`, `filetext.dat`, `i_mapfeature.dat`, and `i_tile.dat` are cached in memory after their first package read, avoiding repeated JVM/archive crossings during map or skill transitions.
3. **Graphics settings are user-controlled again.** Phase 8.16 silently forced shadow/weather/critical effects off in `envinfo.dat`. Phase 8.19 removes that override; the game now persists exactly what the player selects.

Expected markers include:

- `[PHASE8_19_INOTIA2_INSTALL_FOOTER]`
- `[PHASE8_19_INOTIA2_INSTALLED_FASTPATH]`
- `[PHASE8_19_INOTIA2_RESOURCE_CACHE]`

The unsafe Phase 8.17 direct installer-call bypass remains removed.

## Inotia 1 (010100D3 / PD005362)

Phase 8.18 proved that the client consumes the complete 27-byte synthetic command-1 frame, then rejects its semantics and closes the socket before emitting command 2. Phase 8.19 keeps the same deterministic offline response so the rejection can be reproduced, but records the guest CPU/call site immediately before `MC_netSocketClose`/`MC_netClose` resets the bridge.

Expected marker:

`[PHASE8_19_INOTIA1_CASH_REJECT]`

It includes guest PC, LR, R10, R0-R3, receive phase/offset, and 16 bytes around the LR call site when readable. This should identify whether the client rejects the response at its decode/integrity check, command check, or subsequent semantic validation, allowing the next phase to correct the packet rather than guess.

No historical network service is contacted. No cash purchase or save data is modified by this diagnostic phase.
