# WIPI Player Phase 8.48 — Inotia1 entity base-reward spawn trace

Phase 8.48 is based directly on Phase 8.47. It preserves the exact player EXP watchpoint, established Inotia1/Inotia2 compatibility behavior, save/revival paths, and the confirmed 10-record offline cash catalog with `힘의 조각` and `마법의 가지`. The new trace is read-only.

## Why the trace moved upstream

Phase 8.47 conclusively captured both final EXP writes at `0x00171040`: `97,567 -> 98,800` (`+1,233`) for `수호자 C44` and `98,800 -> 97,621` (`-1,179`) for `수호물 K34`. Both writes used the same native store path at `0x00126b62`; the signed reward was already present in `r7`, proving the final EXP store itself is not creating the bad sign.

The same field log showed a fixed live-entity stride of `0x424`: slot 6 at `0x00172918` carried base value `0x00000c64` (`+3172`) for `수호자 C44`, and slot 7 at `0x00172d3c` carried `0xfffff425` (`-3035`) for `수호물 K34`. The final awards are approximately the same normal scaling of those base values. The next target is therefore the code that creates word 0 of each entity object.

## Entity reward watchpoint

When the existing diagnostic button is armed, Phase 8.48:

- logs the current EXP baseline at `0x00171040`;
- snapshots nonzero word-0 values for entity slots 1..31;
- watches word 0 of `0x00171040 + slot * 0x424` for slots 1..31;
- captures 8-bit, 16-bit, and 32-bit guest writes to those fields;
- captures host/bulk writes that overlap an entity word-0 field;
- logs raw signed-32 and low-16/signed-16 interpretations;
- logs PC/LR/SP/r0-r12, 80 bytes of nearby native code, 24 stack words, and 16 words from the affected entity;
- keeps a separate 512-event entity-base budget so the generic 600-event context trace cannot suppress these writes.

The key markers are `PHASE8_48_INOTIA1_ENTITY_REWARD_BASELINE`, `PHASE8_48_INOTIA1_ENTITY_REWARD_WRITE`, `PHASE8_48_INOTIA1_ENTITY_REWARD_CODE`, `PHASE8_48_INOTIA1_ENTITY_REWARD_STACK`, `PHASE8_48_INOTIA1_ENTITY_REWARD_OBJECT`, and `PHASE8_48_INOTIA1_ENTITY_REWARD_SOURCE`.

## What the next field test should answer

If the negative base value is created by a signed 16-bit load, we expect a write such as `0x00000000 -> 0xfffff425` where the source register already contains `0xfffff425`, and the nearby code should reveal the load/sign-extension path. If the entity first receives a positive or packed value and later becomes negative, the sequence of exact slot-base writes will identify the transformation stage instead.

## Recommended test

1. Load the save.
2. Stand immediately before a map transition that will create/recreate monsters, preferably into the area containing the known test monsters.
3. Open Settings > Diagnostics and press **Arm/Reset EXP + Spawn Trace**.
4. Change maps or otherwise force the monsters to spawn.
5. Do **not** kill anything unless convenient; combat is no longer necessary for this diagnostic.
6. Export the log after the monsters appear.

If no map transition is convenient, arming and then causing a monster respawn/reload is also useful.
