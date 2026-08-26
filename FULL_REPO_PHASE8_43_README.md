# WIPI Player Phase 8.43 — Inotia1 EXP / Monster Diagnostic

Phase 8.43 is a diagnostic-only build based directly on Phase 8.42. It is intended to investigate the reported EXP decrease after killing `마력의 생물` and `수호물 k34`, and to determine whether the affected monsters share an internal monster/template/type identifier.

## What changed

- Keeps Phase 8.42 gameplay, save, cash-shop, resurrection, and database-performance behavior unchanged.
- Enables a read-only 32-bit gameplay-stat write observer only for Inotia1 (`AID 010100D3`, `PID PD005362`).
- Records plausible positive and negative stat changes without modifying the guest value.
- For every candidate write, records the destination address, old/new values, signed delta, ARM PC/LR/SP, R0-R12, nearby memory words, nearby native code bytes, and up to four plausible live object heads referenced by registers (48 bytes / 12 words each).
- Caps detailed candidate logging at 320 events per emulator session to prevent runaway logs.
- Leaves the Phase 8.42 memory-write hot path unchanged for every title other than this exact Inotia1 build.

## Diagnostic markers

- `PHASE8_43_RUNTIME_SENTINEL`
- `PHASE8_43_INOTIA1_EXP_TRACE_ARMED`
- `PHASE8_43_INOTIA1_EXP_CANDIDATE`
- `PHASE8_43_INOTIA1_EXP_AROUND`
- `PHASE8_43_INOTIA1_EXP_CODE`
- `PHASE8_43_INOTIA1_OBJECT_HEAD`
- `PHASE8_43_INOTIA1_EXP_TRACE_LIMIT` if the session reaches the safety cap

## Candidate filter

The observer considers an in-page 32-bit write a candidate when:

- diagnostics are enabled for the exact Inotia1 AID/PID;
- the write originates from the Inotia1 client-code address window (`0x00100000`–`0x001fffff`);
- the old and new values are both between 4,096 and 50,000,000;
- the value actually changes; and
- the absolute change is at most 250,000.

This deliberately captures both gains and losses. A normal-monster kill is the control path; an affected-monster kill should let us compare the same memory address and native callsite with the opposite-sign EXP change.

## Recommended field test

1. Build/install TestFlight version `0.1.43` and launch Inotia1.
2. Confirm the exported log contains `PHASE8_43_RUNTIME_SENTINEL` and `PHASE8_43_INOTIA1_EXP_TRACE_ARMED`.
3. Back up the current game save before intentionally killing an affected monster.
4. Reach the test area, then clear the app's visible debug log if convenient so the exported file is short. Clearing the visible log does not need to reset the diagnostic event number.
5. Note the displayed EXP immediately before and after each kill if the game exposes it.
6. Prefer this sequence with minimal unrelated activity between kills:
   - one ordinary monster;
   - `마력의 생물`;
   - one ordinary monster;
   - `수호물 k34`.
7. Export the log immediately afterward and provide it for comparison.

If killing both affected monsters would risk meaningful progress, one affected monster plus at least one normal control monster is enough for the first pass.

## What we will look for in the log

The strongest EXP candidate should have a stable destination address across kills and deltas that match the visible EXP change. Once that address/callsite is identified, the register/object snapshots can be grouped by kill to look for a common monster object field, template ID, type flag, sprite-family pointer, or reward value shared by `마력의 생물` and `수호물 k34`.

This phase intentionally does **not** clamp, reverse, or suppress negative EXP. The diagnostic must preserve the original behavior so the cause can be identified before any compatibility fix is implemented.
