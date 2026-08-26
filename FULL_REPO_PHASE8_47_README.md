# WIPI Player Phase 8.47 — exact Inotia1 EXP watchpoint + reward-signature trace

Phase 8.47 is based directly on Phase 8.46. It preserves the established Inotia1/Inotia2 compatibility paths and the confirmed 10-record offline cash catalog. No gameplay value is modified by the new diagnostic.

## Why this phase is different

The Phase 8.46 field test supplied external ground truth for the main character `이노티아(도적)`: EXP was 97,567 before `수호자 C44`, 98,800 after it (+1,233), and 97,621 after `수호물 K34` (-1,179). The Phase 8.46 object snapshots repeatedly showed `0x00017d1f` (97,567) as word 0 of the live object at `0x00171040`. This phase therefore stops relying on broad candidate heuristics for the final EXP store and watches that exact 4-byte word directly.

## Exact EXP watchpoint

When the user presses **Arm/Reset EXP Trace**, the ARM core logs the baseline value at `0x00171040`. Every subsequent write overlapping that four-byte word is captured independently from the broad event limit and callsite caps. Coverage includes guest 8-bit, 16-bit, and 32-bit stores plus host/bulk `mem_write` operations. This is intentional because 97,567 (`0x00017d1f`) to 98,800 (`0x000181f0`) changes multiple bytes and could be implemented as byte-wise writes.

Exact-write records include:

- complete EXP before/after and signed delta;
- store address and store size;
- PC-before, PC-after, LR, SP, and r0-r12;
- 64 bytes around the native store instruction;
- 24 stack words around SP;
- 24 words from the main-character object beginning at `0x00171040`;
- low-12 and signed-12 interpretations of every live register.

The low-12 trace is diagnostic only. The observed -1,179 equals signed-12 `0xB65`, so Phase 8.47 tests whether a packed 12-bit reward is being sign-extended incorrectly; it does not assume that hypothesis is correct.

## Secondary filtered context trace

The Phase 8.46 filtered 16/32-bit candidate logger remains available for nearby monster/player state context. The known RGB565 writer at guest PC `0x001069c2` remains suppressed, exact address/callsite repeats remain capped, and the generic event budget remains 600. Saturating the generic trace does **not** stop the exact EXP watchpoint.

## Cash catalog

Unchanged from Phase 8.46. The normal offline catalog has 10 records: the proven first eight utility items plus `힘의 조각` and `마법의 가지`. The four prior equipment/cosmetic tail entries remain removed. The emergency `부활의 기도문` party-wipe catalog is unchanged.

## Recommended field test

1. Load the same save and stand next to a monster.
2. Open Settings > Diagnostics and press **Arm/Reset EXP Trace**.
3. Kill a monster and note the displayed EXP before/after if convenient.
4. Kill a second monster and note the displayed EXP again.
5. Export the diagnostic log immediately.

Expected markers: `PHASE8_47_INOTIA1_EXP_BASELINE`, `PHASE8_47_INOTIA1_EXP_EXACT_WRITE`, `PHASE8_47_INOTIA1_EXP_EXACT_CODE`, `PHASE8_47_INOTIA1_EXP_EXACT_STACK`, `PHASE8_47_INOTIA1_PLAYER_OBJECT`, and `PHASE8_47_INOTIA1_EXP_LOW12_SIGNATURE`.
