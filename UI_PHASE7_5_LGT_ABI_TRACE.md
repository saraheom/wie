# Phase 7.5 — LGT ABI Compatibility Trace

This build is diagnostic-only. It does not guess implementations for unknown LGT ABI calls.

## Target failures

- LGT stdlib import `0x3F7` (seen in an Inotia 2 LGT revision)
- LGT stdlib import `0x416` (seen in MapleStory and other LGT titles)
- LGT WIPIC SVC `0x19C` / decimal `412` (seen in an Inotia 2 LGT revision)

## Trace emitted before the existing fatal error

For every unknown stdlib/WIPIC dispatch, WIE now logs:

- exact ABI ID in hex/decimal
- PC, LR, SP, CPSR
- R0-R12
- 12 stack words
- 16 words around the caller return address
- safe 8-word previews for pointer-like R0-R3 values

Trace lines use the prefix `[LGT_ABI]`.

The original fatal behavior is intentionally preserved so no guessed return value can corrupt game state.

## Recommended tests

1. MapleStory Signus: create/load Slot 1 until `0x416`.
2. Inotia 2 LGT v01.00.08: run until first fatal ABI call.
3. Inotia 2 LGT v01.00.04: run until first fatal ABI call.
4. Export the global log and search for `[LGT_ABI]`.
