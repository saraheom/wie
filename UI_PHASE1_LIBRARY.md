# UI Phase 1 — Persistent Game Library

This phase turns the one-shot file picker into a persistent WIPI Player library.

## Implemented

- One-time ZIP/JAR import into IndexedDB (`wipi_player_library`).
- Library cover grid shown whenever WIPI Player opens.
- Tap a cover to launch the stored game without reimporting.
- SHA-256 archive identity prevents accidental duplicate imports.
- Per-game metadata record with future display/control settings fields.
- Rename library entries.
- Import a custom cover image per game.
- Remove a game package/cover from the library.
- Existing WIE IndexedDB filesystem/database save storage is preserved.
- Return-to-library button cleanly stops the current WASM emulator instance.
- MIDI/PCM volume preferences persist between launches.
- Existing desktop keyboard controls and iOS pointer keypad controls are preserved.

## Intentionally deferred

The next UI phases will add:

1. Portrait/landscape player rotation and per-game display sizing.
2. Drag/resize/reposition keypad editor with global and per-game presets.
3. Save-data manager (backup/restore/reset) after exposing a reliable WIE app/save namespace.
4. More library artwork options, screenshots, and game-specific settings panels.

Deleting a game from the Phase 1 library intentionally does not erase WIE's in-game save data.
