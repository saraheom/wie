# Phase 7.16 — Inotia 1 full save-record replacement

Target title: Inotia 1 / KTF PID `PD005362`

## Why this phase exists

Exported local backups captured three real `save0.dat1` generations:

- before Terry
- after Terry
- after the follow-up NPC

They prove that the save record is being rewritten by the game, while the
Phase 7.14 `preserve existing CREATE` workaround is not valid WIPI CREATE
semantics.

## Behavior changes

1. `MC_DB_CREATE` again means create/truncate for Inotia 1.
2. A large Inotia 1 `save*.dat` write beginning at offset 0 is treated as a
   full record replacement. The logical record length is shrunk to the end
   of that write, preventing stale bytes from an older generation from
   surviving past the new payload.
3. Later writes at nonzero offsets still extend/overlay normally. This
   preserves the game's ability to append/update a footer after the main
   save body.
4. All non-Inotia titles keep the existing KTF stream behavior.
5. Added `[INOTIA1_SAVE]` INFO diagnostics for OPEN, READ, WRITE, SEEK,
   SELECT/SEEK, STAT and DELETE_RECORD.

## Test

Start from a clean Inotia 1 save state:
1. Create Slot 1 and save before talking to Terry.
2. Exit to the game title and confirm Slot 1 exists.
3. Reload, talk to 경비병 테리, receive/progress the first quest, and save.
4. Exit to the game title and confirm Slot 1 exists.
5. Relaunch from the WIPI Player library and confirm again.
6. Export the global diagnostic log if the slot still disappears.

Look for `[INOTIA1_SAVE]`.
