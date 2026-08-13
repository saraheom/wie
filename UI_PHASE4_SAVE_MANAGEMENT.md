# UI Phase 4 — Controls + Save Management

This repository combines the working Phase 3 control editor with per-game save management.

## Save-management features

From a game card's `...` menu or from the in-game Settings panel:

- Create a local backup of the game's current normal in-game save data.
- Keep multiple dated backups inside WIPI Player.
- Restore a backup.
- Delete an individual backup without touching the current save.
- Export a backup as `.wipisave.json` using the iOS share sheet / Files app.
- Import a previously exported WIPI Player backup.
- Erase the game's current in-game save while keeping WIPI Player backups.

## How game save storage is identified

WIE stores persistence in IndexedDB. The frontend observes the storage that the active
WIE emulator actually opens and associates it with that library game.

It tracks:

- per-application WIE record databases (`wie_<app-id>`)
- the shared `wie_filesystem` database, filtered by the current game's AID namespace

This is intended to prevent save operations for one library game from clearing another
game's storage.

## Important distinction

This manages the game's normal in-game save data. It does not yet implement arbitrary
emulator freeze states / instant save states; those require VM serialization in WIE.

## Suggested test

1. Launch a game and use its own Save function.
2. Settings → Manage Saves → Create Backup.
3. Change progress and save again.
4. Restore the older backup.
5. Relaunch and confirm the old progress returned.
6. Export a backup to Files, delete that local backup, re-import the exported file,
   then restore it.
7. Test Erase Current In-Game Save only after making a backup.

The temporary diagnostic logger remains enabled and records save discovery and operations.
