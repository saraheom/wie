# WIPI Player Phase 8.45 — manually armed Inotia1 EXP diagnostic

Phase 8.45 preserves the Phase 8.42 gameplay baseline and changes only the temporary Inotia1 EXP diagnostic introduced in 8.43/8.44.

The 8.44 field log proved that the widened 16/32-bit store watcher works, but it reached all 480 events during startup before gameplay. Phase 8.45 therefore starts the watcher **disarmed**. In the in-game Settings > Diagnostics section, use **Arm/Reset EXP Trace** only after the save is loaded and you are positioned near the monsters you want to test.

When armed, the native candidate counter and saturation state are reset. Up to four observations are retained for each exact address + native callsite + write width, preventing hot animation/state counters from consuming the entire trace. The overall detailed-event budget is 600. The diagnostic remains read-only and never changes the guest value.

## Expected markers

- `PHASE8_45_RUNTIME_SENTINEL`
- `PHASE8_45_INOTIA1_EXP_TRACE_AVAILABLE` — title recognized; watcher is still off
- `PHASE8_45_INOTIA1_EXP_TRACE_UI_ARMED` — UI button was pressed
- `PHASE8_45_INOTIA1_EXP_TRACE_MANUALLY_ARMED` — native counter/reset completed and capture began
- `PHASE8_45_INOTIA1_EXP_CANDIDATE` — candidate 16/32-bit value write
- `PHASE8_45_INOTIA1_EXP_AROUND`
- `PHASE8_45_INOTIA1_EXP_CODE`
- `PHASE8_45_INOTIA1_OBJECT_HEAD`
- `PHASE8_45_INOTIA1_EXP_TRACE_LIMIT` — 600 retained events reached

## Test procedure

1. Install TestFlight version `0.1.45`.
2. Launch Inotia1 and load the desired save normally.
3. Walk next to the monsters to test.
4. Open the player Settings panel and press **Arm/Reset EXP Trace**.
5. Close Settings and immediately kill one normal/control monster and one suspicious monster.
6. Export the diagnostic log and share it for analysis.
7. If the trace-limit marker appears before the kills, re-arm immediately before the final hit and repeat with only one monster.
