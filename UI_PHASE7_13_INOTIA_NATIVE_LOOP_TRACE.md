# Phase 7.13 — Inotia 2 Native Loop Trace

Phase 7.13 keeps the Phase 7.12 LGT certificate compatibility patch and adds a
generic ARM long-running-function trace.

## Why

After the Phase 7.12 certificate bypass, both known LGT Inotia 2 revisions finish
startup and enter thread 2 without error 3100 or a WIE fatal exception, but no
longer make WIPI API calls and remain on a black screen. The KTF builds similarly
enter thread 2 after successful storage initialization and then stop producing
useful API-level diagnostics.

## Trace behavior

`ArmCore::run_function` now counts consecutive 1000-instruction engine slices that
complete without a return or SVC. At 64, 256, 1024, 4096, and 16384 consecutive
slices it emits `[NATIVE_LOOP]` diagnostics containing:

- function entry address
- approximate executed instruction count
- PC / LR / SP / CPSR
- R0-R12
- 48 bytes of guest code around PC
- 12 stack words from SP

The counter resets whenever the guest enters an SVC, so normal WIPI-heavy game
execution does not produce these samples. This is a generic emulator diagnostic
and is not keyed to Inotia 2 or MapleStory.

## Regression safety

No API return values, networking behavior, storage behavior, save behavior, or
binary compatibility patches are changed in this phase. MapleStory behavior from
Phase 7.11 remains unchanged. The Phase 7.12 Inotia 2 LGT certificate bypass is
retained.

## Test

Run each failing Inotia 2 build for at least 15-30 seconds. Export the global log
and search for `[NATIVE_LOOP]`. The sampled PC and code window should identify the
next blocking loop even when no WIPI service is called.
