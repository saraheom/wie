# WIPI Player Phase 8.80 — Generic Virtual JAR + High-Retention Diagnostics

Phase 8.80 generalizes the immutable archive fix that was previously limited to OZ. The 8.79.1 Blade Master 3 log showed a 1,331,494-byte read of `000262F4.jar` entering `FileInputStream.read([BII)` without returning, matching the iOS persistent-filesystem stall previously observed with OZ.

## Runtime changes

- Any read-only `.jar` that is already mounted in `FilesystemOverlay::virtual_files` now uses the in-memory archive for existence, metadata/length, and content reads.
- New markers: `PHASE8_80_GENERIC_VIRTUAL_JAR_EXISTS`, `PHASE8_80_GENERIC_VIRTUAL_JAR_METADATA`, `PHASE8_80_GENERIC_VIRTUAL_JAR_READ_BEGIN`, `PHASE8_80_GENERIC_VIRTUAL_JAR_READ_RETURN`, and `...FALLBACK`.
- OZ `/kpool`, `wie.rustjar` negative-cache behavior, Blade Master 3 safe compositing/media compatibility, and Chrono Swing `0xD9` compatibility are retained.
- Persistent filesystem semantics remain unchanged for writable files, saves, databases, and non-JAR resources.

## Diagnostic changes

- Global retained history increased to 16,000 lines / 4 MB.
- Each launched game additionally gets an independent persisted 8,000-line / 2 MB history. Export includes the global history plus clearly separated per-game sections.
- High-value `PHASE*`, `GAME`, and window-error messages are synchronously saved as the last breadcrumb. The next app launch emits `PHASE8_80_RECOVERED_LAST_BREADCRUMB`, so a force-close/freeze is less likely to erase the final known boundary.
- `PHASE8_80_GAME_TEST_BEGIN` and `PHASE8_80_DIAGNOSTIC_SCOPE` identify each test run.

TestFlight marketing version: **0.1.80**.
