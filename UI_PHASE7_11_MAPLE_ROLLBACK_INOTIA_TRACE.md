# Phase 7.11 — MapleStory Regression Rollback + Inotia 2 Focus Trace

## MapleStory rollback

Phase 7.11 is based on Phase 7.8, the last TestFlight build confirmed to run
MapleStory successfully through verification/gameplay/save.

It intentionally removes the two later broad experiments introduced in 7.9/7.10:

- `MC_netConnect()` no longer forces a success callback. It is restored to the
  prior stub behavior (`M_E_ERROR`) that MapleStory already handled correctly.
- LGT archives no longer globally replace WIPI phone/model/ESN properties with
  archived download metadata. System-property behavior returns to the known
  working Phase 7.8 baseline.
- The speculative LGT WIPIC `0x266` success override is removed with the rollback.

All proven pre-existing compatibility work remains: 0x416/memcmp, 0x3F7/sprintf,
0x19C database capacity, 0xCF graphics GetContext, 16 MiB virtual storage, save
persistence, archive normalization, Korean UI, and diagnostics.

## Inotia 2 trace

The LGT Inotia 2 builds currently reach thread 2 and repeatedly format error 3100
without a preceding fatal WIE exception. Phase 7.11 adds a one-shot diagnostic:

    [INOTIA_TRACE] first error=3100 ...
    [INOTIA_TRACE] error=3100 caller=... caller_words_base=... words=[...]
    [INOTIA_TRACE] error=3100 stack_words=[...]

Only the first occurrence per app runtime is expanded, preventing the global log
from being overwhelmed while preserving the exact caller context needed for the
next compatibility fix.

The KTF Inotia 2 diagnostics from earlier phases remain intact; this phase does
not add another speculative KTF memory behavior.
