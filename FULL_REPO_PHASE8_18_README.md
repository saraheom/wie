# WIPI Player Phase 8.18 — Dual Inotia recovery

Phase 8.18 is a full-repository release built on Phase 8.17.

## Inotia 2 (010100D5 / PD007974)

Phase 8.17's direct branch bypass at guest 0x001780E2 is removed. Field testing showed that the native rebuild routine also initializes required in-memory resource tables; skipping it produced the game's `메모리에러` screen.

Phase 8.18 keeps the required native initialization but removes its worst emulator-side cost:

- static mode-4 install records (`i_pack.dat`, `eventdata.dat`, `filetext.dat`, `i_mapfeature.dat`, `i_tile.dat`) are rebuilt in guest memory;
- their previous canonical repository copies are preserved while rebuilding;
- buffers are preallocated to the known canonical sizes;
- per-write repository flushes and full-buffer snapshots are skipped;
- each static record is committed once at database close;
- generated caches are normalized to the shipped expanded `p/` copy;
- normal-open canonical resource fast paths from Phase 8.17 remain enabled;
- shadow, weather and critical-effect settings remain forced off;
- the Inotia-2-only ARM execution quantum returns to the known-working 4,000 instruction slice.

Expected diagnostics include:

- `[PHASE8_18_INOTIA2_EXEC_QUANTUM]`
- `[PHASE8_18_INOTIA2_INSTALL_WRITEBACK] OPEN ...`
- `[PHASE8_18_INOTIA2_INSTALL_WRITEBACK] PREALLOC ...`
- `[PHASE8_18_INOTIA2_INSTALL_WRITEBACK] CLOSE ...`

The unsafe `[PHASE8_17_INOTIA2_INSTALL_CALL_BYPASS]` marker must not be present.

## Inotia 1 (010100D3 / PD005362)

Phase 8.17 recovered the game's authentic first cash-shop request:

`00 14 01 0B 30 31 30 31 32 33 34 39 38 37 36 01 00 00 00 64`

Phase 8.18 retains the command-0 bootstrap and adds the first local command-1 server response. The response follows the native parser's verified field layout and uses status 0 rather than any of the explicit legacy error statuses 1003/1004/1009.

Expected diagnostics:

- `[PHASE8_17_INOTIA1_CASH_CMD0_BOOTSTRAP]`
- `[PHASE8_16_INOTIA1_NET31_TX]`
- `[PHASE8_12_CASH_TX]`
- `[PHASE8_18_INOTIA1_CASH_INIT_RX]`
- subsequent `[PHASE8_18_INOTIA1_CASH_PROTOCOL]` lines for any next request.

This phase is intentionally still protocol-preserving. It does not contact the historical server. Later purchase commands are captured rather than guessed, so a zero-cost purchase response can be implemented against the real packet contract without risking save/inventory corruption.
