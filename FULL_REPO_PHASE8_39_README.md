# WIPI Player Phase 8.39 — Inotia1 Party-Wipe Latch + Blessed-Revival Fault Trace

Phase 8.39 is based on the field-validated Phase 8.37 gameplay/performance/catalog baseline and the Phase 8.38 party-wipe static analysis. It does not change normal movement scheduling, the normal 12-item catalog, the `이노티아` main-name repair, Phase7_21 save seek behavior, or Inotia2.

## Party-wipe cash cancellation

The Phase 8.38 field log proves the missing-prayer flow first reaches command 5 with native death state 14, then reconnects two seconds later after the title has changed the outer state to 6. Phase 8.38 recomputed the emergency flag at each command 5, so the second reconnect downgraded `true` to `false`. CLEAR/back therefore had no emergency context left, and no command-123 packet was emitted during the observed stuck sequence.

Phase 8.39 latches the emergency origin once state 14 is observed. The latch survives reconnects and socket close. Every command-30 request in that latched flow continues to receive the one-item `부활의 기도문` catalog.

Because the observed CLEAR/error dismissal does not emit command 123, Phase 8.39 also adds a narrow CLEAR-key recovery: only while the emergency latch is active, CLEAR writes the exact native death-prompt destination (state 11, selection 0) and then forwards CLEAR normally to the guest. No ordinary movement key is instrumented. Command-123 retains a second recovery path if the title does emit it.

## 축복받은 부활주문서 crash

The supplied log contains one fatal `Invalid memory access; address: 596` after normal cash cleanup, matching the reported blessed-revival crash, but the legacy error text does not preserve the guest PC/LR. Phase 8.39 therefore adds exception-only ARM fault context. It logs PC/LR/SP/R0-R3 only when an invalid-memory fault is already occurring. There is no per-frame/per-key diagnostic and no normal gameplay overhead. The next reproduction will distinguish a low-PC control-flow fault from a data-memory fault and identify the exact native instruction for the following fix.

## Test order

1. Verify TestFlight 0.1.39 and `PHASE8_39_RUNTIME_SENTINEL`.
2. Reproduce full-party death, choose `부활의 기도문`, choose the 120-resource route, and press CLEAR when the cash error appears. Confirm it returns to the death prompt instead of hanging.
3. Separately use `축복받은 부활주문서` on one dead hero. If it still crashes, export the log; `PHASE8_39_ARM_FAULT_CONTEXT` is the key line.
