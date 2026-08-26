# WIPI Player Phase 8.44 — Inotia1 widened EXP-store diagnostic

Phase 8.44 is a diagnostic-only successor to Phase 8.43, still preserving the Phase 8.42 gameplay baseline. The first 8.43 field log contained no `EXP_CANDIDATE` events while monsters were killed, proving that its 32-bit-only / >=4096 filter did not see the real EXP update path.

## What changed

- Keeps Phase 8.42 gameplay, save, cash-shop, resurrection, and database-performance behavior unchanged.
- Enables the observer only for Inotia1 (`AID 010100D3`, `PID PD005362`).
- Observes both 16-bit (`STRH`/`w16`) and 32-bit (`STR`/`w32`) guest stores.
- Removes the old `>=4096` value floor so small current-EXP values and changes around ~150 remain visible.
- Adds `width=16` or `width=32` to every candidate event.
- Records address, old/new values, signed delta, ARM PC/LR/SP, R0-R12, nearby memory, nearby native code, and up to four plausible live object heads.
- Caps detailed candidate logging at 480 events per emulator session.
- Never changes, clamps, reverses, or suppresses the value written by the game.

## Noise filters

To keep the widened trace usable without reinstating a high EXP floor, a store is ignored when:

- it does not originate from Inotia1 native client code (`0x00100000`–`0x001fffff`);
- old or new is zero (startup/initialization suppression);
- the absolute delta is 0 or 1;
- both values are <=31 (tiny state/animation counters);
- the destination is in the observed stack page (`0x400f0000`–`0x400fffff`);
- a 16-bit delta exceeds 30,000; or
- a 32-bit value exceeds 50,000,000, its delta exceeds 250,000, or both old/new values look like aligned guest pointers.

These filters are diagnostic heuristics only; they do not alter game execution.

## Diagnostic markers

- `PHASE8_44_RUNTIME_SENTINEL`
- `PHASE8_44_INOTIA1_EXP_TRACE_ARMED`
- `PHASE8_44_INOTIA1_EXP_CANDIDATE` (includes `width=16|32`)
- `PHASE8_44_INOTIA1_EXP_AROUND`
- `PHASE8_44_INOTIA1_EXP_CODE`
- `PHASE8_44_INOTIA1_OBJECT_HEAD`
- `PHASE8_44_INOTIA1_EXP_TRACE_LIMIT` if the session reaches 480 detailed events

## Recommended field test

1. Build/install TestFlight version `0.1.44` and launch Inotia1.
2. Play normally until you reach the monster test area, then clear the visible app debug log if convenient.
3. Kill only a few monsters initially: one ordinary/control monster and one monster known to give suspicious EXP are sufficient for the first pass.
4. Export the log immediately after those kills.
5. If the log contains `PHASE8_44_INOTIA1_EXP_CANDIDATE`, send it for analysis before doing a large multi-monster test.
6. If it contains `PHASE8_44_INOTIA1_EXP_TRACE_LIMIT`, send the log as-is; do not spend more EXP.

You do not need to manually record every before/after EXP value. An ordered monster-name list is still helpful because the diagnostic does not yet know the human-readable monster name reliably.

## What we will look for

The first goal is to identify a stable EXP destination address and callsite whose delta matches the visible EXP change. Once that is known, the same PC/LR/register/object snapshots can be used to trace backward into the reward calculation and the packaged `P/mon.dat` monster records.
