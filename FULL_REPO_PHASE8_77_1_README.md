# Phase 8.77.1 CI Fix

This is a compiler-only correction to Phase 8.77.

- Fixes Rust E0499 in `wie_wipi_c/src/api/graphics.rs` by evaluating the Blade Master 3 title predicate before `framebuffer.canvas(context)` takes its mutable borrow.
- Keeps the Phase 8.77 Blade Master 3 image diagnostics unchanged.
- Keeps the OZ `/kpool` virtual-file probe/fix unchanged.
- Keeps the Phase 8.75 OZ virtual JAR path unchanged.
- Keeps the Phase 8.76 Chronoswing WIPIC 0xD9 compatibility handler unchanged.
- TestFlight version remains 0.1.77 because the failed build never uploaded.
