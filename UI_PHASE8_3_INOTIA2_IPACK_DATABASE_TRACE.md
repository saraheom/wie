# Phase 8.3 — Inotia 2 i_pack database-path trace

This phase preserves:
- Phase 8.2 Inotia 2 heap/storage diagnostics.
- Phase 8.1.2 future-proof Inotia 1 save-length fix.

## What Phase 8.2 proved

The visible `메모리에러` is not caused by:
- exhaustion of Inotia 2's internal 1 MiB allocator, or
- the startup database-storage capacity gate.

The storage check reports roughly 16.7 MiB available against a 10 KiB
requirement, so it passes by a very large margin.

Static disassembly then narrows the visible error state:

    0x1780dc -> call 0x1450bc
    if that returns 0 -> state 2
    otherwise call 0x144f48
    if that returns 0 -> state 2

State 2 renders the CP949 strings at 0x18e7a8 / 0x18e7b4:
`메모리에러` / `OK: 종료`.

Because Phase 8.2 observes the final successful branch inside 0x1450bc,
the remaining failure candidate is 0x144f48.

## Why trace the DB path

0x144f48 calls helper 0x1439ac. Disassembly shows 0x1439ac creates
`i_pack.dat`, and a later helper reopens/validates it. A mismatch in KTF
stream-database semantics can therefore make 0x144f48 return 0 while the
game misleadingly displays a generic memory error.

## New diagnostics

For PID PD007974, Phase 8.3 logs database operations at INFO level:

    [PHASE8_3] Inotia2 i_pack database trace active
    [INOTIA2_DB] OPEN_REQUEST ...
    [INOTIA2_DB] OPEN_RESULT ...
    [INOTIA2_DB] WRITE_BEGIN ...
    [INOTIA2_DB] WRITE_RESULT ...
    [INOTIA2_DB] READ_BEGIN ...
    [INOTIA2_DB] READ_RESULT ...
    [INOTIA2_DB] SELECT_SEEK ...
    [INOTIA2_DB] CLOSE ...
    [INOTIA2_DB] STAT ...
    [INOTIA2_DB] EXISTS_KTF ...

No database return values or guest state are changed in this phase.
