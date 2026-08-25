# WIPI Player Phase 8.37 — Inotia 1 performance rollback + complete safe cash catalog

Phase 8.37 is based on the field-tested Phase 8.36 TestFlight runtime.

## Validated result retained

Phase 8.36 confirmed that the exact native callsite repair restores the main character from `자원 교환권(도적)` to `이노티아(도적)` on both the Continue screen and the in-game menus. That exact repair is retained unchanged. The secondary hero remains untouched.

## Performance rollback

Phase 8.35 removed none of the expensive broad name instrumentation soon enough, and Phase 8.36 then added a PC/LR read plus diagnostic log on every Inotia1 key-down to investigate the independent party-wipe issue. Directional movement therefore still paid diagnostic overhead on every input.

Phase 8.37 restores the normal Inotia1 input path: `handle_event` directly queues the event again. There is no per-keydown PC/LR read/log, no broad heap/GOT name scan, no repaint-time native scan, and no generic name-string trace. The only active name compatibility code is the exact validated main-name repair at LR `0x0010cf29` / `0x0010cf5b`. Inotia2 scheduler/graphics/LZMA behavior is unchanged.

## Cash shop

The client has exactly 12 safe catalog slots. Phase 8.28 only corrupted adjacent character-name storage when records 13/14 were written, so Phase 8.37 uses all 12 valid slots but never writes a 13th record.

The 12 records are:

1. 스킬북
2. 부활주문서
3. 축복받은 부활주문서
4. 상자 열쇠
5. 축복받은 용사의 인장
6. 16칸 가방
7. 스킬 초기화
8. 초보용 용사의 인장
9. 흑기사의 투구
10. 레게 스타일
11. 번개 스타일
12. 스텔스 가면

The smaller bag variants and normal 용사의 인장 are omitted as redundant, and the server-only 자원 교환권 remains excluded. Existing 자원 교환권 in an old save still receives the offline failure response rather than fake “10 acquired” success.

Phase 8.36's `[0,0]` page metadata still caused an initial `error occurred` dialog even though clearing it allowed the catalog to display. Phase 8.37 uses the previously working Phase 8.33 page-0 compatibility metadata `[0,1]`. All command-30 requests map to the same physical 12-record catalog. No hidden second set of items exists, so arrow navigation is not required to reach any item.

## Party wipe

The party-wipe input probe is removed from this production-performance test. The total-party-death stall remains a separate compatibility issue and Phase 8.37 deliberately does not auto-revive, consume a scroll, or force the main menu without stronger evidence of the original game's intended transition.

## TestFlight identity

- Marketing version: `0.1.37`
- Runtime sentinel: `[PHASE8_37_RUNTIME_SENTINEL]`
- Build marker: `PHASE8_37_BUILD_SENTINEL=WIPI_PLAYER_0.1.37_PERFORMANCE_BASELINE_COMPLETE_CATALOG`
