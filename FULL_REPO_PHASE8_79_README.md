# Phase 8.79 — High-information OZ + Blade Master 3 compatibility

This phase is intentionally designed to reduce TestFlight iteration count.

- Retains Phase 8.75/8.77 OZ virtual JAR and /kpool reads.
- Retains Phase 8.78 negative `wie.rustjar` metadata cache.
- Adds `PHASE8_79_OZ_CLASSPATH_STAGE` for the post-rustjar boundary.
- Retains Chronos Wing 0xD9 compatibility unchanged.
- Adds Blade Master 3 media-player shadow state for alloc/set/get/free player calls.
- Adds title-gated entry/return tracing for every BM3 LGT WIPIC SVC.
- Hardens final WebView framebuffer presentation: buffer-length validation, magenta sampling, and no panicking `unwrap()` at ImageData/putImageData.
- Adds Rust and TypeScript frame/update-stage diagnostics around opaque WASM traps.
- Expands diagnostic retention to 12,000 lines / 3,000,000 chars.

TestFlight marketing version: 0.1.79.
