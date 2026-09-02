# WIPI Player Phase 8.78

Phase 8.78 keeps the working Phase 8.76 Chronos Swing 0xD9 compatibility handler and all OZ virtual-file fixes.

## OZ
The synthetic WIE classpath entry `wie.rustjar` is now explicitly pre-seeded as a negative metadata result in addition to `RT_RUSTJAR`. This prevents the iOS/web asynchronous VFS `size()` lookup seen hanging in Phase 8.77.1. Expected marker: `PHASE8_78_OZ_WIE_RUSTJAR_NEGATIVE_HIT`.

## Blade Master 3
For exact AID `000262F4` / PID `PD109653`, 32-bit ARGB images drawn into the 16-bit LCD framebuffer use a title-gated bounds-safe per-pixel alpha-composite path instead of the generic mixed-format blitter. Expected markers: `PHASE8_78_BM3_SAFE_ARGB_TO_RGB565_BEGIN`, `...RETURN`, and `PHASE8_78_BM3_DRAW_COMPLETE`.

TestFlight marketing version: 0.1.78.
