# Phase 8.48 note

The current TestFlight workflow targets WIPI Player 0.1.48. Phase 8.48 is based directly on Phase 8.47 and keeps the confirmed 10-record Inotia1 cash catalog unchanged, including `힘의 조각` and `마법의 가지`, while preserving the exact main-character EXP watchpoint at `0x00171040`.

The new diagnostic targets the upstream monster/entity base-reward field discovered by the Phase 8.47 field test. Inotia1 live entities are observed at `0x00171040 + slot * 0x424`; slot 6 (`0x00172918`) carried `수호자 C44` base reward `+3172`, while slot 7 (`0x00172d3c`) carried `수호물 K34` base reward `-3035` before final EXP scaling. When manually armed, Phase 8.48 snapshots word 0 of entity slots 1..31 and captures every subsequent 8/16/32-bit write to those fields, with signed-16 provenance, PC/LR/registers, native code, stack, and entity data.

For the next test, arm the trace before a map transition or other monster-spawn event and export after the target monsters appear. Killing them is not required. See `FULL_REPO_PHASE8_48_README.md`.
