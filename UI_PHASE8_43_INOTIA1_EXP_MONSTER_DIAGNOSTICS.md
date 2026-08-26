# Phase 8.43 UI / TestFlight diagnostic notes

No production UI behavior is changed in this phase.

The existing app debug-log/export flow is reused. Phase 8.43 adds native/WASM log records beginning with `PHASE8_43_INOTIA1_...` so a field log can correlate candidate EXP writes with the native instruction and nearby game objects.

Suggested test labels to include when sharing a log:

- ordinary control monster #1
- 마력의 생물
- ordinary control monster #2
- 수호물 k34
- displayed EXP before/after each kill, when practical

The diagnostic is intentionally temporary and should be removed or disabled after the EXP/reward path is identified.
