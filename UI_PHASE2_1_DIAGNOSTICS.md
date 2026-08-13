# UI Phase 2.1 — Immediate Rotation Reflow + Temporary Diagnostic Logs

## Rotation reflow fix

The landscape keypad layout no longer depends on CSS `(orientation: landscape)`.
The selected per-game `data-orientation` is now the source of truth, so D-pad and
numeric keypad placement changes immediately when the rotate control is pressed.

The player also refreshes layout on:

- window resize
- orientationchange
- Screen Orientation API `change`
- VisualViewport resize
- completion/failure of an orientation-lock request

A synchronous WebKit layout refresh is used to avoid delayed `display: contents`
and CSS-grid recomputation during iOS rotation animations.

## Temporary diagnostic logging

Game Settings now contains a **Diagnostics (testing only)** section:

- View Log
- Export Log
- Clear

Captured events include:

- console log/info/warn/error/debug
- uncaught JavaScript errors
- unhandled promise rejections
- application visibility/background transitions
- game imports and launches
- display/orientation changes
- layout refreshes

The log is bounded and persisted locally for testing. Export uses the iOS share
sheet when supported; choose **Save to Files** to create a `.txt` file that can
be uploaded for debugging. On desktop browsers it falls back to a normal file
download.

This diagnostic feature is intentionally temporary and should be removed or
disabled before a public release.
