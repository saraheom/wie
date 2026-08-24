# Phase 8.34 — Inotia1 field-test targets

## 1. Cash-shop page navigation

Phase 8.34 corrects the command-30 wire order using the native parser/UI code:

- first byte after result = `page_count` -> GOT `0x510`
- second byte = zero-based `current_page` -> GOT `0x4a4`
- third byte = `record_count` -> GOT `0x4d8`

The renderer at guest `0x0014cd1a` uses `current_page + 1` against `page_count`. The click path at `0x0014c7a8` / `0x0014c7c4` increments/decrements the current page and bounds it against page count. Earlier phases sent the first two values in reverse order, which explains the impossible `3/0`-style UI state and the blocked forward direction.

Expected Phase 8.34 frames:

- page 0: `... 1e 01 02 00 09 ...`
- page 1: `... 1e 01 02 01 08 ...`

Both page directions should now operate through the game's original logic.

## 2. Cash-shop description cleanup

When the title emits `00 04 7b 00`, the bridge queues `00 04 7b 01`. Static dispatch maps server command 123 to the native generic cleanup handler at guest `0x001171fa`. Check whether the last cash-item description no longer appears after entering Inventory.

Marker: `PHASE8_34_INOTIA1_CASH_EXIT_CLEANUP`.

## 3. Resource-exchange ticket

`자원 교환권` is omitted from the offline catalog. Existing tickets receive command-89 result/state 0 rather than the old synthetic success. The ticket represents a historical server/account resource balance and has no normal single-player inventory target.

Marker: `PHASE8_34_INOTIA1_RESOURCE_EXCHANGE_OFFLINE_ONLY`.

## 4. Corrupted character names

The Phase 8.33 16/32-byte heap probe found no candidates. Phase 8.34 checks mapped runtime data/BSS, all in-use WIE allocation classes, and one-level GOT-reachable structures before local catalog injection. It auto-repairs base names only if the pair is uniquely present in mapped native runtime state. Exact RVCT `strcpy`/`strlen` calls also report corrupt base/display strings; complete `name(class)` strings can be safely repaired in place.

Markers:

- `PHASE8_34_INOTIA1_NAME_BOUNDED_PROBE`
- `PHASE8_34_INOTIA1_CHARACTER_NAME_REPAIR`
- `PHASE8_34_INOTIA1_NAME_STRING_CALL`
- `PHASE8_34_INOTIA1_DISPLAY_NAME_REPAIR`
