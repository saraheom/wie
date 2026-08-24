# WIPI Player Phase 8.33 full-repository notes

## Scope

Phase 8.33 is an Inotia 1 field-fix phase. Inotia 2 code paths are intentionally unchanged from Phase 8.32/8.30-era behavior so its separate startup/performance validation can continue independently.

## 1. Cash shop: two safe compatibility pages

The Phase 8.32 log proves that the remaining page failure is guest state, not a lost network response. After page 0, the working page control sends request 1 and receives page 1 correctly. While the UI is stuck at 3/1, repeated taps keep transmitting request 1. Request 2 appears only after entering and backing out of an item-detail view. The opposite page control produces no useful transition.

Phase 8.33 therefore stops requiring a third native page. All 18 zero-cost catalog records are distributed as 9 + 9. Nine records remains below the Phase 8.28 fixed-array overflow point (records 13/14 caused the corruption). Response page metadata is now page 0/1 with max_page_index=1. Any nonzero request, including stale request 2 after item-detail/back, returns page 1.

Markers:
- `[PHASE8_33_INOTIA1_FIRST_OPEN_TWO_PAGE]`
- `[PHASE8_33_INOTIA1_CASH_TWO_PAGE]`

## 2. Character names: live heap probe before catalog allocation

Phase 8.32's catalog-array-adjacent repair never fired in field testing. That establishes that the old overflow location is the source of the corruption, but not the post-relaunch storage location of the serialized names.

At the first authentic command-5 shop-transfer request, Phase 8.33 inspects only WIE's allocated 16-byte and 32-byte small-object buckets. This timing is deliberate: the gameplay save is already loaded, but the synthetic catalog names have not yet been copied into guest heap allocations. The probe searches for the exact EUC-KR corrupt pair supplied by field testing:

- `자원 교환권` -> `이노티아`
- `초보용 용사의 인장` -> `기사`

It also recognizes the exact rendered forms `자원 교환권(도적)` and `초보용 용사의 인장(기사)`. A repair is performed only when both members of a base pair or both members of a rendered pair are unique. Otherwise the build logs all candidate addresses and makes no write. No opaque `save0.dat` bytes are patched.

Markers:
- `[PHASE8_33_INOTIA1_NAME_HEAP_PROBE]`
- `[PHASE8_33_INOTIA1_CHARACTER_NAME_REPAIR]`

## 3. Preserved validated behavior

Phase 8.30 resource-exchange command 89 handling and the Blessed Seal network-state bypass are unchanged. Existing save seek/record-length compatibility is unchanged.

## 4. Stale cash-item description

The Phase 8.32 field run still reports the last cash-item description appearing after entering inventory. The same run does not emit the previously observed command-123 reset marker, so Phase 8.33 does not fabricate a network-exit response. The two-page change removes the malformed third-page transition first. If the overlay remains, the next log should be used to identify the guest screen-stack/selection state directly.

## Suggested field test

1. Launch the corrupted current save and confirm the two scenario names before opening the cash shop.
2. Open the cash shop once. Check the log for the heap probe and repair markers, then return to the scenario screen and check whether names changed.
3. Verify the shop starts at 2/0 and the working page control reaches 2/1. All 18 items should now be split between those two pages.
4. Open/back out of an item detail and confirm it remains on a valid 2/0 or 2/1 page rather than producing a hidden third page.
5. Exit to inventory and report whether the stale description still appears.
6. If names were repaired, make a normal in-game save, relaunch, and confirm whether the corrected names persist.
