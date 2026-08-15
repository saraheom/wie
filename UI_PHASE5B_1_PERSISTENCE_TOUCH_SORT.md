# Phase 5B.1 — Save Persistence + Fast Favorite + Sort Direction

## Save persistence

WIE IndexedDB writes now resolve only when the IndexedDB transaction reaches `oncomplete`,
not merely when the individual `put()` request reports success. Pending WIE writes are tracked
and awaited before the emulator is freed when returning to the library. The app also attempts a
best-effort pending-write flush when it moves to the background.

Diagnostic logs now contain `[SAVE_IO] write committed ...` entries and background pending-write
counts so future save regressions can be distinguished from game-side save behavior.

## Favorite interaction

The cover-star button uses `pointerup` for immediate iOS response, prevents the underlying cover
button from receiving the gesture, and updates the star in-place. A full library rerender only
happens when the active sort mode is Favorites.

## Home sorting

The home sort control has two independent interactions:

- Tap the arrow button to reverse the current order.
- Tap the mode dropdown to choose Recently Played, Name, or Favorites.

Direction is persisted globally. Name defaults to A→Z; Recently Played and Favorites default to
newest/favorites-first.
