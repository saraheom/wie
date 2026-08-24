# WIPI Player Phase 8.27 — Inotia 1 first-open special catalog + Inotia 2 corrected LZMA fast path

Phase 8.27 is a full-repository release built on Phase 8.26.

## Inotia 1: first-click shop and expanded offline catalog

Phase 8.26 proved the command-30/31 protocol end-to-end: the local command-30 record for `단검` resolved through the title's own item table, rendered its original icon/details, and the command-31 success response let the original game add the item to inventory and serialize its save.

The surviving client does not embed the historical server-side cash catalog/order/prices. Phase 8.27 therefore does not pretend to reproduce unavailable server metadata. It exposes a client-authentic offline special-item catalog using exact item names that are present in the 2007 `work.bar` item database, all with value/price 0:

- 스킬북
- 부활주문서
- 축복받은 부활주문서
- 상자 열쇠
- 용사의 인장
- 축복받은 용사의 인장
- 3칸 가방
- 6칸 가방
- 9칸 가방
- 12칸 가방
- 16칸 가방
- 스킬 초기화
- 자원 교환권
- 초보용 용사의 인장

The game still performs all item lookup, icon/description rendering, inventory mutation, and save serialization itself.

Phase 8.27 also makes the catalog immediately pending when the command-2/command-4 transfer finishes. The existing async read callback wakes it on the first shop entry instead of waiting several seconds for the title's later command-30 refresh. A later authentic command-30 request still receives the same catalog normally.

Markers:

- `[PHASE8_27_INOTIA1_FIRST_OPEN]`
- `[PHASE8_27_INOTIA1_CASH_CATALOG]`
- `[PHASE8_27_INOTIA1_CASH_PURCHASE]`

## Inotia 2: corrected private-header LZMA acceleration

The Phase 8.26 field log proved the hook was installed but immediately disabled itself with `header/output length mismatch`. Exact disassembly of guest `0x00125928` and the packaged resource bytes resolves why: the title does not pass a normal LZMA-Alone header to the wrapper.

At wrapper entry, relative to guest `r0`:

- bytes 1..5: canonical 5-byte LZMA property block;
- bytes 6..9: little-endian u32 unpacked length (the same value passed in r3);
- bytes 10..13: title-private metadata;
- byte 14 onward: raw LZMA payload.

Phase 8.27 synthesizes a temporary standard LZMA-Alone header in host memory using the original 5-byte properties and `r3` as the u64 unpacked size, appends the untouched payload, then uses `lzma-rs` to decompress directly into the guest's original caller-allocated buffer.

This reconstruction was independently validated against five original packaged resources: `eventdata.dat`, `filetext.dat`, `i_mapfeature.dat`, `i_tile.dat`, and `game.dat`. For the four files with persisted `P/` counterparts, the decoded bytes exactly match the canonical file prefix; the title's normal +8-byte trailer remains guest-owned.

The Phase 8.25 RGB565 fast path remains unchanged because current field testing reports smooth in-game movement, skills, map changes, and in-game menus.

Marker:

- `[PHASE8_27_INOTIA2_LZMA_FASTPATH]`

The installer/initializer itself is still executed; only its expensive LZMA work is accelerated. The Phase 8.22 progress renderer suppression remains in place.
