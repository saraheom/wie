# Phase 8.3.1 — Inotia 2 i_pack pre-rebuild checkpoint

This phase preserves:
- Phase 8.3 database tracing;
- Phase 8.2 heap/storage diagnostics;
- Phase 8.1.2 future-proof Inotia 1 save-length fix.

## Why Phase 8.3 produced no DB trace

Static disassembly now shows that `0x1439AC` performs a guest-heap allocation
before its first WIPI database open call.

The exact path is:

    0x1450BC  -> success
    0x17812E  -> 0x144F48
    0x144F8A  -> 0x1439AC
    0x1439D4  -> allocator((count - 1) * 4)
    0x1439DC  -> if NULL, return 0 immediately
    0x1439FC  -> only then open/create i_pack.dat

Therefore an allocation failure at `0x1439D4` prevents every Phase 8.3
`[INOTIA2_DB]` log line from appearing.

## What Phase 8.3.1 logs

At the already-confirmed `MC_dbListDataBase` checkpoint immediately before
`0x144F48`, it logs:

- whether packaged `i_pack.dat` is visible to WIE;
- packaged length and first 16 bytes;
- packaged header version and count;
- the guest globals populated by `0x143A88`;
- the count used by the pre-open rebuild allocator;
- the exact `(count - 1) * 4` allocation request;
- whether that request fits the currently free guest heap.

Markers:

    [PHASE8_3_1]
    [INOTIA2_IPACK_PRE]

No guest memory, database data, or return values are changed.
