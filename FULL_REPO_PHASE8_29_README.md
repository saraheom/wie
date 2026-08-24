# WIPI Player Phase 8.29 — Inotia 1 safe cash pagination + network-special item use

Phase 8.29 is a full-repository release based on Phase 8.28.

## Inotia 1 — character-name corruption fix

Phase 8.28 sent all 18 offline cash-shop records in one command-30 frame. Field
screenshots revealed that character names were replaced by catalog records 13
and 14 (`자원 교환권` and `초보용 용사의 인장`). That exact boundary identifies
the original client catalog as a fixed 12-record-per-page structure; records
13+ overflowed the adjacent in-memory character-name storage.

Phase 8.29 never sends more than 12 records in one page. The same 18-item
catalog is split into:

- page 1: 12 original special/utility entries;
- page 2: `자원 교환권`, `초보용 용사의 인장`, `흑기사의 투구`,
  `레게 스타일`, `번개 스타일`, `스텔스 가면`.

The original command-30 request's final page byte selects page 0/1. Responses
also advertise page index and two total pages. Invalid page values safely fall
back to page 0.

Markers:

- `[PHASE8_29_INOTIA1_CASH_CATALOG_PAGE]`
- `[PHASE8_29_INOTIA1_FIRST_OPEN_SAFE_PAGE]`

This prevents new catalog parsing from overwriting character-name memory. If a
name was already persisted to save data by an earlier build, do not guess the
old name; restore it from a backup or recover it from an uncorrupted save copy.

## Inotia 1 — network-only consumables in single-player

Phase 8.28 bypassed the first network-state==2 gate at guest `0x0015032e`.
Static analysis shows a second branch at guest `0x0015034a` that sends the
contiguous network-special item-ID range `0xF4..0xFE` back to the same error
2001 handler. This is why `축복받은 용사의 인장` and related network-only
consumables still reported that they could only be used in Network Mode.

Phase 8.29 NOPs only that exact guarded BLS branch for the exact Inotia 1
AID/PID/native image. The game's original item-use, consumption, stat/resource
update, and save logic remain unchanged.

Marker:

- `[PHASE8_29_INOTIA1_NETWORK_SPECIAL_USE_GATE]`

## Inotia 2

No Inotia 2 behavior is changed in Phase 8.29. The Phase 8.27 LZMA acceleration
and Phase 8.28 all-effects RGB565 batching remain intact.
