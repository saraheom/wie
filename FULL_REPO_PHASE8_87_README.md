# WIPI Player Phase 8.87

Phase 8.87 builds on the Phase 8.86.1 baseline.

## OZ
- Repairs malformed LGT missing-vtable class-name resolution by accepting one validated pointer indirection when direct UTF-8 is invalid.
- If neither direct nor indirect names are valid, the malformed missing-vtable descriptor is logged and neutral-returned instead of converting descriptor corruption into a fatal `net/wie/WieError`.
- New markers: `PHASE8_87_LGT_VTABLE_CLASS_RESOLVED` and `PHASE8_87_OZ_MALFORMED_VTABLE_DESCRIPTOR_BYPASS`.

## Blade Master 3
- Instruments the actual MIDP display paint path, including `paintDisabled`, screen-image dimensions/bpp/raw length, and RGB565 magenta count.
- For the exact BM3 title, a requested redraw is allowed to present the MIDP screen image even when Clet `paintDisabled` is set, as a safe title-scoped fallback.
- Adds lower-level web redraw-request breadcrumbs.

All earlier compatibility fixes, including the Chronoswing WIPIC 0xD9 compatibility path and BM3 synchronous record I/O, remain intact.
