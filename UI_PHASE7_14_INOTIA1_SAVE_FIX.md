# Phase 7.14 — Inotia 1 Save Persistence Fix

## Diagnosis

The Inotia 1 diagnostic log shows that the game successfully commits
`save0.dat1` (380 bytes) into the WIE IndexedDB-backed database for PID
`PD005362`.

The remaining failure is consistent with Inotia 1 reopening that existing
save database with WIPI `MC_DB_CREATE` mode as part of normal slot handling.
WIE previously treated every CREATE open as a truncate and deleted record 1
before reopening it.

## Fix

For PID `PD005362` only:

- an existing database reopened with mode 4 / `MC_DB_CREATE` is preserved
  instead of being truncated;
- existing record bytes seed the new database handle normally;
- additional `[INOTIA1_SAVE]` diagnostics record:
  - preserved CREATE reopens;
  - write-through operations;
  - explicit record deletions.

All other application IDs retain the previous database semantics.

This phase otherwise remains based on Phase 7.13.
