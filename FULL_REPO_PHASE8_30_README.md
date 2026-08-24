# WIPI Player Phase 8.30 — Inotia 1 network consumables + Inotia 2 startup/menu probe

Phase 8.30 is a full-repository release built on Phase 8.29.

## Inotia 1 — 자원 교환권 authentic command 89

The Phase 8.29 field log proves that the existing single-player network-use
patches are active. `자원 교환권` now gets past those local checks, opens the
offline network bridge, and emits the title's authentic seven-byte request:

`00 07 59 00 02 00 0a`

The client then times out with cash/network error 2014 only because the bridge
had no response for command 89.

Static analysis of the exact PD005362 native image resolves receive command 89
at guest `0x00119294`. The handler consumes the common response state byte and,
when that value is 1, continues through the game's original resource/inventory
update path. It does not consume a command-specific payload. Phase 8.30 therefore
queues the minimal faithful local response:

`00 04 59 01`

Marker:

`[PHASE8_30_INOTIA1_RESOURCE_EXCHANGE]`

No inventory record or resource balance is directly edited by WIE; the guest's
original command-89 completion code remains responsible for the effect and save.

## Inotia 1 — 축복받은 용사의 인장 remaining local gate

Phase 8.29 removed the network-special ID-range rejection at guest `0x0015034a`,
but the item still reports that it can only be used in Network Mode and emits no
network request. A second exact network-state check is reached through the
item/type path at guest `0x001485b6`. For an item resolving to type `0xBA`, the
client requires global network state 2 before calling the original item-use
routine; otherwise it produces the network-only failure UI.

For the exact Inotia 1 AID/PID/native-size match, Phase 8.30 changes only that
`BEQ` into the same unconditional branch to the existing valid continuation.
All item identification and effect code remains original guest code.

Marker:

`[PHASE8_30_INOTIA1_BLESSED_SEAL_USE_GATE]`

If the item subsequently emits a server command, that command remains observable
and will be emulated from the authentic packet contract rather than guessed.

## Inotia 1 — persisted character names

Phase 8.29 prevents the original cash-shop fixed-array overflow by limiting each
catalog page to at most 12 records. The currently corrupted character names,
however, survived a complete relaunch, which means they were already persisted
by the earlier oversized-catalog build. Phase 8.30 deliberately does **not**
guess or rewrite character names automatically. Restoring those names safely
requires either an older uncorrupted save/backup or the original character names
so a targeted repair can be validated against the save structure.

## Inotia 2 — startup / main-menu isolation probe

The Phase 8.29 log confirms both current host accelerators are active:

- corrected LZMA resource decompression executes successfully;
- the all-row RGB565 batch fast path is installed.

Yet, on a second launch, `i_pack.dat` finishes its required initialization
writeback and approximately 7.8 seconds elapse before the first full-screen
RGB565 batch and authentication sequence. This remaining black interval is
therefore not explained by the already-accelerated first LZMA decode or by the
few-millisecond persistent writes.

Phase 8.30 intentionally makes **no new execution or graphics optimization** to
avoid regressing the now-stable in-game path. It retains the 4,000-instruction
Inotia-2-only execution slice and enables a one-shot native-loop diagnostic at
2,048 chunks (~8.2 million guest instructions) for this exact title. Each
`run_function` can emit at most one equality-threshold diagnostic, providing the
PC/LR of the long black-startup or main-menu work without re-enabling the old
per-frame stall profiler.

Marker:

`[PHASE8_30_INOTIA2_STARTUP_MENU_PROBE]`

The next field log should identify the exact guest routine to accelerate while
leaving smooth gameplay and the required initializer unchanged.
