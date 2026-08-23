# WIPI Player Phase 8.24 — Inotia 1 transfer finalization/re-entry + Inotia 2 quiet interpreter optimization

Phase 8.24 is a full-repository release built on Phase 8.23.

## Inotia 1 (010100D3 / PD005362)

Phase 8.23 made the first major cash-shop protocol breakthrough: the original title accepted the corrected command-0/command-1 handshake, emitted authentic outbound command 5, and consumed the local command-2 transfer-start response. Field testing then showed a highlighted/partially initialized UI over the normal equipment screen. The same session later emitted outbound command 123 followed by command 30, and the second cash-shop attempt remained on `Connecting...` because neither command had a local continuation.

Phase 8.24 addresses those two protocol-state gaps without fabricating inventory or purchases.

### 1. Finish the empty catalog-transfer sequence

Command 5 now queues two contiguous server frames:

- command 2: successful zero-byte transfer start
- command 4: successful empty transfer finalizer

The legacy receive bridge automatically advances from command 2 to command 4 on the next guest read, instead of returning `M_E_WOULDBLOCK` between the two frames. This mirrors a continuous TCP stream and allows the original native cash-shop state machine to finish its transfer path instead of being left halfway initialized.

Markers:

- `[PHASE8_24_INOTIA1_CASH_TRANSFER_SEQUENCE]`

The command-4 frame is deliberately empty. It does **not** create catalog items, grant currency, change `char.dat`, or synthesize a purchase result.

### 2. Make re-entry deterministic

The observed outbound `00 04 7b 00` (command 123) has no corresponding receive-dispatch handler in this native build, so it is treated as a one-way reset/cancel marker and clears only the local response queue.

When the title then emits its authentic command-30 handset/session request, Phase 8.24 supplies a minimal command-30 success frame with zero records. Static analysis of the dedicated command-30 receive handler shows that its success path consumes three fixed one-byte fields before entering a count-controlled record loop; a zero third field safely avoids that loop while allowing the original completion path to run.

Markers:

- `[PHASE8_24_INOTIA1_CASH_REENTRY]`

The next objective is still to reconstruct the actual catalog-record format after the protocol/UI state is stable. The cash shop is **not yet an offline free-item catalog** in this phase.

## Inotia 2 (010100D5 / PD007974)

Phase 8.23 was intentionally diagnostic. It lowered native-loop tracing to roughly 4.1 million guest instructions and logged every frame gap >=40 ms. The field log produced hundreds of frame-stall records and identified the remaining long hitches as guest-native CPU work, especially software pixel/color-conversion loops around guest `0x00123f82`, rather than the final Web/iOS presentation bridge.

Phase 8.24 converts that diagnostic build back toward a production-performance build:

1. The Inotia-2-only execution slice remains at the known-good 4,000 instructions.
2. Phase 8.23's per-frame stall/present timing and low native-loop trace threshold are removed from the hot path. This eliminates a large amount of WebView/console logging overhead during ordinary animation and combat.
3. The ARM32 interpreter dispatch loop now reuses the PC value it already fetched for each guest instruction. Previously it fetched PC once in `run()` and again inside the SVC-exception check on every instruction. CPSR is now read only if PC is actually the SVC exception vector. This is behavior-preserving but removes one register lookup from every interpreted instruction, which matters in the multi-million-instruction software blit loops identified by Phase 8.23.
4. Phase 8.22's RGB565/raw framebuffer and Web canvas fast-paint changes remain active.

Marker:

- `[PHASE8_24_INOTIA2_QUIET_PERF]`

### Installation/startup

The visible installation progress renderer remains suppressed, but the required native initializer still runs. Therefore a black startup interval remains possible. We do not restore the old direct initializer bypass because field testing proved that skipping the routine causes `메모리에러` by omitting required in-memory resource-table initialization.

### What Phase 8.24 can tell us

If the long skill/map hitches are materially reduced after removing profiler overhead and the interpreter dispatch optimization, then interpreter cost was a significant contributor. If the large hitches remain, the next targeted optimization should be a guarded host acceleration/hook for the exact Inotia 2 software color-conversion/blit routine around guest `0x00123fxx`, rather than more scheduler or filesystem changes.

## Scope

All compatibility behavior remains title-scoped. No intentional behavior changes are made for MapleStory, Heroes Lore 2, or other WIPI titles.
