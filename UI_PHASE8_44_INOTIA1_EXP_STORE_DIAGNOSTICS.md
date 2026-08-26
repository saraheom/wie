# Phase 8.44 UI / TestFlight diagnostic notes

No user-facing controls or gameplay behavior are changed. The existing app debug-log/export flow is reused. Phase 8.44 adds widened native/WASM log records beginning with `PHASE8_44_INOTIA1_...`. Candidate lines now include `width=16` or `width=32`.

This build is intentionally temporary and read-only. It exists to locate the actual EXP store/callsite before any compatibility repair is attempted.
