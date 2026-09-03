# WIPI Player Phase 8.84

## Blade Master 3 — synchronous record I/O
Phase 8.83 proved that `OpenDatabase("setup.dat")` can return entirely from the guest-backed session-memory database, but the next `WriteRecordSingle (0x192)` still fell through to the generic persistent repository. Phase 8.84 keeps BM3 record reads/writes in the same guest-backed session-memory domain. `stream_write` updates the guest buffer plus session mirror and returns before `open_db_for_handle()`/IndexedDB. Markers: `PHASE8_84_BM3_DB_STREAM_WRITE_SYNC` and `PHASE8_84_BM3_DB_STREAM_READ_SYNC`.

## OZ — first-loop guest execution forensics
The Phase 8.83 Java-side PC was the WIE dispatcher trampoline rather than the native OZ callsite. Phase 8.84 adds a generic opt-in 128-entry guest-PC history in the ARM engine, enabled only by the LGT OZ frontend. On the first `java/lang/Exception.<get-initialized-class>` retry from LR `0x17b70`, WIE freezes the recent guest PC/opcode sequence, register/stack state, and static callsite window. Repetitive 10,000-call loop milestones are suppressed after the first forensic snapshot so the useful pre-loop context stays in the exported log. Markers: `PHASE8_84_OZ_FIRST_LOOP_GUEST_PC_TRACE`, `PHASE8_84_OZ_FIRST_LOOP_REGS`, `PHASE8_84_OZ_CALLSITE_CODE_WINDOW`, and `PHASE8_84_OZ_EXCEPTION_LOOP_SANITY`.

TestFlight marketing version: **0.1.84**.
