# Phase 7.15 — Inotia 1 Save Mirror Synchronization

## New evidence

A clean Inotia 1 run produced two successful persistent writes to `save0.dat1`:

- 320 bytes immediately after character creation;
- 328 bytes after accepting the first quest and moving north of town.

The second save therefore reaches persistent storage. The slot disappears during the game's subsequent load/validation path rather than because the host write failed.

## Root emulator inconsistency

KTF's database implementation mirrors record 1 inside each open `DatabaseHandle` for the stream-style read/write ABI.

`stream_write()` updated both this mirror and persistent storage. However, standard `MC_dbUpdateRecord()` updated only persistent storage. A title that performs `UpdateRecord(1, ...)` and then reads through the still-open stream handle sees stale bytes from before the update.

This is especially visible in Inotia 1 when its save grows from 320 to 328 bytes after the first quest.

## Fix

`update_record()` now synchronizes record 1 into the open handle after a successful repository update:

- reallocates the guest mirror when needed;
- copies the replacement record into the mirror;
- sets `buffer_len` to the replacement size (including shrink cases);
- clamps read/write cursors to the new record length;
- writes the updated handle back to guest memory.

For Inotia 1 (`PD005362`) it also emits:

`[INOTIA1_SAVE] update_record mirror-sync ...`

The synchronization is generic database correctness, not an Inotia-specific behavior override.

## Regression test

Added a test that writes record 1, replaces it via `update_record()`, seeks the same open handle back to zero, and verifies `stream_read()` returns the replacement bytes.
