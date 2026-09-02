# WIPI Player Phase 8.77

Combined follow-up to Phase 8.76.

- Retains Chrono Swing WIPIC 0xD9 compatibility handler.
- Retains OZ Phase 8.75 direct virtual-memory JAR read.
- OZ: for exact AID 00026DBF / PID PD112525, read-only `/kpool` first attempts the packaged virtual-file bytes. It never fabricates kpool content; if absent, normal persistent fallback remains and is logged.
- Blade Master 3 (AID 000262F4 / PID PD109653): diagnostic-only image creation and draw logging records decoded dimensions/bpp and sampled magenta pixels to localize the bitmap-font corruption path.

Expected OZ markers: `PHASE8_77_OZ_KPOOL_VIRTUAL_READ_BEGIN`, then `...RETURN` if packaged auth data is present, otherwise `...FALLBACK`.
Expected BM3 markers: `PHASE8_77_BM3_IMAGE_CREATE` and `PHASE8_77_BM3_DRAW_IMAGE`.
