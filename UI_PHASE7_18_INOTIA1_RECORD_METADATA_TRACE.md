# Phase 7.18 — Inotia 1 record-metadata trace

Target: KTF Inotia 1, PID `PD005362`.

This is a diagnostic-only phase. It does **not** alter Inotia 1 save bytes,
record length, cursor behavior, or persistence semantics.

## Why

Phase 7.17.1 established a clean transition:

- before Terry: `save0.dat` is written as 320 bytes, and Continue reads 320;
- after Terry: `save0.dat` is written as 324 bytes, but Continue requests only
  320 bytes and then rejects the slot.

The next question is whether the game also consults database metadata/size, or
whether it rejects the first 320 bytes entirely in guest ARM code.

## Added diagnostics

- Build marker: `[PHASE7_18]`.
- Every Inotia 1 `save*.dat` WRITE now logs:
  - full record FNV-1a fingerprint;
  - first-320-byte fingerprint;
  - bytes beyond offset 320.
- Every Inotia 1 `save*.dat` READ now logs:
  - fingerprint of exactly what the guest receives;
  - full backing-record fingerprint;
  - backing first-320 fingerprint;
  - unread byte count;
  - backing bytes beyond offset 320.
- INFO traces for metadata paths:
  - `[INOTIA1_META] LIST_RECORDS`
  - `[INOTIA1_META] LIST_RECORD_INFO`
  - `[INOTIA1_META] EXISTS_STANDARD`
  - `[INOTIA1_META] EXISTS_KTF`
  - `[INOTIA1_META] STAT`

## Test sequence

1. Erase Inotia 1 saved state.
2. Create Slot 1.
3. Save before Terry and confirm Continue shows Slot 1.
4. Load Slot 1, talk to 경비병 테리, accept/progress the first quest, and save.
5. Return immediately to Continue and confirm whether Slot 1 disappears.
6. Exit to WIPI Player and export the global diagnostic log.

Do not restore/import a backup during this run.
