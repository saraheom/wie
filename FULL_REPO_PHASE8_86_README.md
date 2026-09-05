# Phase 8.86 — OZ WieError String + BM3 First Refresh

- TestFlight marketing version: 0.1.86.
- OZ: decodes the real `net/wie/WieError` Java `detailMessage` via the LGT Java object/char-array layout at the exact failing `startApp()` boundary.
- BM3: logs first `Repaint` and `FlushLcd` activity and requests one safe redraw immediately after successful LGT startup to expose/present a dirty first framebuffer.
- Retains all Phase 8.80–8.85 compatibility fixes, including generic virtual-JAR reads, BM3 session DB, and OZ `/kpool`/classpath handling.
