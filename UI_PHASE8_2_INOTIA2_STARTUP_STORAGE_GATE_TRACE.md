# Phase 8.2 — Inotia 2 startup storage-gate trace

This phase preserves:

- the Phase 8.1 Inotia 2 dereferenced heap scan;
- the Phase 8.1.2 future-proof Inotia 1 save-length fix.

## Phase 8.1 result

The KTF Inotia 2 internal allocator is healthy at the observed startup point:

- capacity: 0xFA000 (1,024,000 bytes)
- used: 0x3478 (13,432 bytes)
- free: 0xF6B88
- 5109 free descriptors
- largest contiguous gap: 0xF6B88
- the 0x100-byte allocation succeeded (`ui_ptr` is non-zero)

Therefore the visible `메모리에러` is not caused by exhaustion of that
allocator at this point.

## New static/dynamic correlation

The ARM caller recorded in the Phase 8.1 log was:

    LR = 0x0012301F

Disassembly of `client.bin1149832` shows guest `0x12300C` dispatching
`MC_dbListDataBase`. After it returns, startup routine `0x1450BC` compares the
reported available database storage to:

    r7 + 0x2800

`r7` is callee-saved, so it is still observable while the WIPI SVC is active.

Phase 8.2 logs the exact values:

    [INOTIA2_STORAGE_GATE]
    available=...
    r7_resource_total=...
    required=r7+0x2800=...
    margin=...
    would_pass=true|false

It also records all ARM registers at the call.

## Interpretation

If `would_pass=false`, the startup failure is an emulated KTF database-storage
capacity/reporting mismatch. The next phase can test a title-scoped corrected
storage-capacity value without touching the game's ARM validation.

If `would_pass=true`, this storage gate is not the rejection point. We then
move to the next branch after `0x1450BC` rather than changing memory/database
limits.

No guest state or return value is changed in Phase 8.2.
