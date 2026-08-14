# UI Phase 5B — Library Fixes + Save-State Feasibility Lab

## Library fixes

- Favorite/unfavorite no longer revokes the currently displayed cover URL during a lightweight rerender.
- Cover object URLs are cached by game ID and invalidated only when the cover changes or the game is deleted.
- The home subbar now contains the library sorting control instead of the redundant Import WIPI Entry button.
- The top **Import** button accepts normal ZIP/JAR games and portable `.wipigame.json` entries.
- Sorting remains persistent through the existing app settings store, but is now directly accessible on the home screen.

## Phase 5B save-state feasibility

The current WIE WebAssembly API does not expose a complete runtime serializer. The emulator
contains live CPU engine state, ARM thread contexts, mapped memory, registered handlers,
timers, audio/backend objects, and platform state. The existing Save Manager snapshots only
persistent game storage and is not a full emulator freeze-state.

This phase therefore adds **Experimental State Lab → Inspect Save-State Capability** to the
running-game settings. It records the actual methods exported by the current `WieWeb`
WebAssembly object and whether a serializer-like API is available. The result is also written
to the global diagnostic log.

This deliberately avoids presenting normal game-save backups as true save states. A future
full state implementation will require core changes to pause execution and serialize/restore
CPU registers, memory, thread state, timers, and platform/runtime state safely.
