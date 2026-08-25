# WIPI Player Phase 8.40 — Inotia1 Prayer Purchase Handoff + Resurrection Context Repair

Phase 8.40 is based on the field-validated Phase 8.37 gameplay/performance/catalog baseline and the Phase 8.39 party-wipe reproduction. It does not change normal movement scheduling, the normal 12-item cash catalog, the `이노티아` main-name repair, Phase7_21 save seek behavior, or Inotia2.

## Emergency `부활의 기도문` purchase handoff

The Phase 8.39 field log proves the emergency command-31 purchase succeeds: the local success response is consumed and `save0.dat` grows from 1768 to 1772 bytes. A few seconds later the outer native state is 1. Phase 8.39 still had the pre-purchase emergency latch active, so CLEAR incorrectly rewrote that legitimate post-purchase state from 1 to 11 and stranded the overlay.

Phase 8.40 keeps the state-14 latch only while the emergency purchase is pending. When the complete emergency command-31 success frame is delivered, the latch is cleared and a command-123 cleanup response is left pending. The log proves the guest immediately re-arms `MC_netSetReadCB` after its save commit, so that callback can consume the cleanup without waiting for the 30-second cash timeout. The native command-31 state-13/state-1 path is not overwritten. CLEAR/back after a successful purchase is therefore left to the original game UI. The state-11/selection-0 CLEAR recovery remains only for cancelling before a purchase succeeds.

## Resurrection-scroll crash

Phase 8.39 captured the exact fault:
- data-memory fault address `0x0000024c`
- post-fault PC `0x0011dfcc`; actual faulting load at `0x0011dfca`
- LR `0x00131cb9`
- R2 `0x248`

Static disassembly shows `0x0011dfca` loads through `R4+0x248`, while the callsite at `0x00131cb2` already carries the live character context in R8 and uses its `+0x248/+0x24c` fields. The fault address implies R4 was `4` on the crashing path.

Phase 8.40 adds one hash-keyed, exact Inotia1 hook at the 16-bit `LDR R0,[SP,#0x4c]` immediately before that call. The hook emulates the original LDR, validates that `R8+0x248` and `R8+0x24c` are readable, copies `R8 -> R4`, and continues into the untouched BL. It fires only on this rare native callsite and has no per-frame/per-key overhead. The existing exception-only Phase 8.39 fault trace is retained as a fallback if another fault remains.

Because hash-keyed binary-patch entries take precedence over the generic fallback, the Inotia1-specific entry explicitly retains all compiler-library and register-copy hooks used by the Phase 8.37 performance/name baseline.

## Test targets

1. Verify TestFlight 0.1.40 and `PHASE8_40_RUNTIME_SENTINEL`.
2. With one dead hero, use the resurrection scroll that previously crashed. Look for `PHASE8_40_INOTIA1_REVIVAL_CONTEXT_REPAIR`; the app should no longer terminate at `0x0011dfca`.
3. For a full-party wipe, enter the `부활의 기도문` purchase path and buy it. After command 31, `PHASE8_40_INOTIA1_PRAYER_PURCHASE_HANDOFF` should appear, followed by immediate command-123 cleanup when the guest re-arms its read callback.
4. Do not force state 11 after a successful purchase. If the cash UI still remains visible, press CLEAR once; Phase 8.40 should no longer intercept that post-purchase CLEAR.
5. Confirm normal movement performance and the regular 12-item cash shop remain unchanged.
