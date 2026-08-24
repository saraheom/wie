# WIPI Player Phase 8.32 — Inotia 1 page-bound correction + exact character-name recovery

Phase 8.32 is a full-repository release built directly on Phase 8.31. Inotia 2 behavior is unchanged from the Phase 8.30/8.31 baseline.

## Field evidence from Phase 8.31

The August 24 Phase 8.31 run confirms that both special-item paths remain operational:

- `자원 교환권` emits authentic command 89 (`00 07 59 00 02 00 0a`), receives the minimal success frame `00 04 59 01`, and re-enters the title's own save path.
- `축복받은 용사의 인장` continues through the original item-use path with the exact type-`0xBA` network-state gate bypassed.

The same run also exposes two Phase 8.31 issues. First, command-30 responses advertised byte 5 as `3`; the title displayed `4/0`, later requested page index `3`, and the Phase 8.31 clamp returned page 2 again. Second, the graphics-level character-name render probe did not fire, proving that probe was attached above/beside the actual native name path rather than at the relevant storage operation.

## Inotia 1 — correct page-bound semantics

Static parser analysis plus the field behavior show that the command-30 byte after `page_index` is the **highest valid page index**, not the number of pages. Three physical pages therefore use indices `0`, `1`, and `2` while advertising `max_page_index=2`.

Phase 8.32 keeps the safe six-record pages introduced in Phase 8.31 but changes every page header from `... page_index, 03, 06 ...` to `... page_index, 02, 06 ...`. Requests `0`, `1`, and `2` are honored exactly. An unexpected request above `2` now falls back to page 0 rather than being clamped to page 2, avoiding the Phase 8.31 `4/2` trap.

Markers:

- `[PHASE8_32_INOTIA1_FIRST_OPEN_PAGE_BOUND]`
- `[PHASE8_32_INOTIA1_CASH_PAGE_BOUND]`

Expected UI page labels are `3/0`, `3/1`, and `3/2` using the title's own convention.

## Inotia 1 — exact recovery of the two corrupted character names

The Phase 8.28 single-frame catalog sent 18 records into a native twelve-entry, four-byte pointer table. Static Thumb analysis of the exact PD005362 `client.bin138532` resolves that catalog-name table through GOT slot `r10+0x4b8`. Entries 13 and 14 therefore wrote to the immediately adjacent pointer slots at `table+0x30` and `table+0x34`.

The user-provided clean/corrupt references identify those two persisted values exactly:

- character 1: `이노티아` became `자원 교환권`;
- character 2: `기사` became `초보용 용사의 인장`.

Phase 8.32 does **not** rewrite or splice the opaque `save0.dat`. Instead, at the title's RVCT string-helper hooks it resolves only those two adjacent pointer slots and applies an in-place repair only if the complete NUL-terminated EUC-KR bytes exactly match one of the two known corrupt strings. The replacements are shorter than the existing allocations and the remainder is zero-filled. All other values are untouched.

Marker:

- `[PHASE8_32_INOTIA1_CHARACTER_NAME_REPAIR]`

The title's normal save serializer remains responsible for persisting the corrected names. A unit test builds the same pointer-table layout in mapped guest memory and verifies both exact replacements.

## Cash-shop description state

A deeper pass through the Phase 8.31 log confirms that the title does emit its authentic command-123 exit/cancel marker (`00 04 7b 00`) and the offline bridge resets its pending local-response queue with no reply, exactly as intended. The lingering item-description popup therefore is not evidence of a missing network teardown response; it is a guest-side selection/description state that survives the menu transition.

Phase 8.32 does not fabricate a new server reply for command 123. The page-bound correction is applied first because the malformed phantom-fourth-page state can itself leave the guest UI in an abnormal selection state. If the popup remains after page navigation is corrected, the next diagnostic can target the guest selection/description state directly.

## Preserved behavior

Phase 8.32 intentionally keeps unchanged:

- Phase 8.30 command-89 resource-exchange success bridge;
- Phase 8.30 blessed-seal type-`0xBA` gate bypass;
- earlier Inotia 1 handshake/purchase/callback fixes;
- Phase 8.30/8.31 Inotia 2 startup diagnostic and performance paths.

No historical server is contacted.
