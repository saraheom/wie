# UI Phase 6 — iOS/Mobile Polish

This phase is based on the working Phase 5B.1.1 persistence build.

## Fixes

- Save Manager now keeps its title/close button visible and scrolls the backup content inside the modal.
- Sort direction button now uses a robust iOS touch/click path, toggles immediately, persists the direction, and visibly changes ↑/↓.
- Home header is compacted on iPhone so `WIPI Player` and `＋ Import` stay on one line.

## Phase 6 mobile polish

- Added **Keep screen awake while playing** in Home Settings.
- Uses the Screen Wake Lock API when available, releases the lock when returning to the library, and reacquires it after foregrounding the app.
- Existing save-transaction hardening, favorite quick-tap behavior, global diagnostics, control editor, and save manager are preserved.

No new native Tauri plugin dependency is introduced in this phase, keeping the TestFlight build surface small while save persistence is being validated.
