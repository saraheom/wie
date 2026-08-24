# WIPI Player Phase 8.35

This phase is based on the Phase 8.34 source, but adds build provenance checks because the 2026-08-24 field log identified itself as Phase 8.33 despite the intended Phase 8.34 test.

## Inotia 1 changes
- One cash-shop page only: 9 high-value offline items, fixed client capacity 12, safety margin 3.
- No resource exchange ticket in the catalog; existing command-89 use remains rejected offline.
- Main-character recovery no longer depends on the secondary hero slot: a unique native-runtime `자원 교환권` match is repaired to `이노티아`; the secondary hero is untouched.
- Phase 8.34 command-123 native cleanup response remains.

## Build provenance
- TestFlight version is `0.1.35`.
- Inotia 1 logs `[PHASE8_35_RUNTIME_SENTINEL]` at load.
- CI checks the sentinel in the source, built WASM, and final IPA before App Store upload.

## CI/TestFlight guardrails
- The pre-build sanity step rejects stale multi-page catalog constants and verifies the Phase 8.35 source markers plus app version `0.1.35`.
- The workflow verifies `PHASE8_35_RUNTIME_SENTINEL` inside the built WebAssembly before iOS packaging.
- Before App Store Connect upload, the final IPA is unpacked and must contain both the runtime sentinel and `CFBundleShortVersionString = 0.1.35`.
- This prevents another field test from silently exercising an older Phase 8.33 runtime.
