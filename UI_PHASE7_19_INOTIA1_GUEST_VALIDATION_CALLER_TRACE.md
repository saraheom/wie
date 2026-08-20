# Phase 7.19 — Inotia 1 guest validation caller trace

Phase 7.18 proved:

- Pre-Terry: Inotia writes 320 bytes and Continue receives the identical 320 bytes.
- Post-Terry: Inotia writes 324 bytes; Continue deliberately reads the first 320.
- The post-Terry first-320 fingerprint returned by WIE exactly matches the first
  320 bytes Inotia originally wrote.
- Continue does not query record size/list/stat metadata before rejecting the
  slot; it only performs the KTF existence check.

Therefore Phase 7.19 stops investigating persistence and records the ARM guest
caller state at the exact `save0.dat` Continue read.

## Added diagnostics

`[PHASE7_19]` confirms this build.

`[INOTIA1_ARM]` includes:

- r0-r12, SP, LR, PC, CPSR
- up to 12 saved LR values from the Thumb R7 frame chain
- 64 bytes of guest stack
- code bytes around LR
- code bytes around PC
- record length and unread-byte count

No save bytes, database semantics, registers, or guest memory are modified.

## Test

1. Clean save.
2. Save before Terry and verify Continue shows Slot 1.
3. Load it, talk to 경비병 테리, accept/progress the quest, save.
4. Return to Continue and reproduce the missing Slot 1.
5. Export the global diagnostic log.

The pre- and post-Terry `[INOTIA1_ARM]` records should reveal the native
validation call site. The next phase can then instrument or patch the exact
branch rather than altering WIPI database semantics.
