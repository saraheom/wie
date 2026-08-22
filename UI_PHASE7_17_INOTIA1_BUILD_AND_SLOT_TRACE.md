# Phase 7.17 — Inotia 1 Build Fingerprint and Slot Validation Trace

## Why this phase exists

The Phase 7.16 runtime log for a Slot 3 save shows `save2.dat` being written as exactly 324 bytes, but the Phase 7.16 `full-record replace` INFO marker is absent. Given `cursor=324` and `write_len=324`, the write necessarily began at offset 0, so a Phase 7.16 core should have emitted that marker for PID `PD005362`.

That makes build/runtime verification the first priority before changing database semantics again.

## Changes

- Adds an explicit `phase=7.17` fingerprint to Inotia 1 database diagnostics.
- Logs save-database OPEN calls, including mode, existence and packaged-resource state.
- Logs every save stream write *before* cursor mutation (`STREAM_WRITE_PRE`).
- Logs KTF slot-stat checks (`STAT`) with the record size returned to the game.
- Logs KTF database-existence checks (`EXISTS_KTF`) and their return value.
- TestFlight workflow force-cleans the WASM release target and `wie_web/pkg`/`dist` before the frontend build, then verifies the 7.17 source marker and generated WASM file.

No new guessed save-format behavior is introduced in this phase; the Phase 7.16 replacement-length fix remains in place.
