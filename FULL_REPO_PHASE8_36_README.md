# WIPI Player Phase 8.36 — Inotia 1 cash-entry recovery, low-overhead main-name repair, party-wipe probe

Phase 8.36 is based on the field-tested Phase 8.35.3 TestFlight runtime.

## Field evidence addressed

The Phase 8.35 runtime successfully restored the visible main-character name from `자원 교환권(도적)` to `이노티아(도적)`, but it introduced two regressions:

1. The cash shop completed command 5 and delivered a 174-byte nine-record command-30 frame whose header began `1e 01 01 00 09`. The guest then rejected the catalog and showed `error occurred` instead of entering the item list.
2. Phase 8.35 retained broad name-recovery diagnostics in hot graphics/string paths. The test log contained 149 `PHASE8_34_INOTIA1_NAME_STRING_CALL` events in a short run, plus repeated bounded heap/GOT scans. This instrumentation is removed from runtime execution.

The earlier known-working two-page response used `1e 01 00 01 09` for page zero. This proves the Phase 8.34/8.35 field swap was wrong. Phase 8.36 keeps the safe one-page catalog and uses the compatible one-page state `current_page=0, max_page_index=0`, i.e. header `1e 01 00 00 09`.

## Cash shop

The catalog remains exactly nine records, safely below the original client's 12-record storage capacity:

- 스킬북
- 부활주문서
- 축복받은 부활주문서
- 상자 열쇠
- 용사의 인장
- 축복받은 용사의 인장
- 16칸 가방
- 스킬 초기화
- 초보용 용사의 인장

There is no page navigation and no `자원 교환권` catalog entry. Existing resource-exchange tickets still use the Phase 8.34 offline rejection response rather than the misleading fake-success response.

## Main-character name repair

All broad runtime scans and generic string diagnostics are disabled from active paths. The only repair now runs at the two exact Inotia 1 native `strlen` callers recovered from the successful Phase 8.35 log:

- `LR=0x0010cf29`: exact base string `자원 교환권\0` -> `이노티아\0`
- `LR=0x0010cf5b`: exact displayed string `자원 교환권(도적)\0` -> `이노티아(도적)\0`

The helper reads memory only when one of those exact callers is active and writes only after a complete exact-byte match. The secondary hero is intentionally untouched.

## Party-wipe issue

The user's earlier gameplay log showed a total-party-death stall that predates Phase 8.35. There was no new `NATIVE_LOOP` warning at the actual stall; guest activity became quiet for roughly 87 seconds before later save activity. Phase 8.36 does not guess whether the original title should auto-revive or return to the menu.

For the next reproduction, the exact Inotia 1 emulator logs only key-down events with the current guest PC/LR:

`[PHASE8_36_INOTIA1_PARTY_WIPE_INPUT_PROBE]`

This diagnostic does not alter input delivery, combat, death, saving, or resurrection behavior.

## TestFlight identity

- Marketing version: `0.1.36`
- Runtime sentinel: `[PHASE8_36_RUNTIME_SENTINEL]`
- The Phase 8.35.3 CFBundleVersion verification logic is retained, updated only for version 0.1.36.
