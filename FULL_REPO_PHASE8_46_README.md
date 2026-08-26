# WIPI Player Phase 8.46 — filtered EXP trace + resource-material cash catalog

Phase 8.46 is based directly on Phase 8.45. It keeps the EXP diagnostic read-only and preserves the established Inotia1/Inotia2 compatibility paths.

## EXP diagnostic

The Phase 8.45 field log proved the manual arm path works but also identified a dominant 16-bit writer at guest PC `0x001069c2`: it consumed 596/600 retained events while touching hundreds of different `0x4036...` heap addresses in roughly 76 ms. Phase 8.46 suppresses only that observed 16-bit callsite rather than excluding the allocator region. It also caps every remaining `PC + width` pair at 24 retained events and keeps the existing exact `address + PC + width` cap of four.

Test: load the save, stand near the monsters, Settings > Diagnostics > **Arm/Reset EXP Trace**, kill two monsters, then export the log.

## Cash catalog

The normal offline catalog is now 10 records, below the proven native capacity of 12. The first eight utility items remain unchanged. The four equipment/cosmetic tail entries (`흑기사의 투구`, `레게 스타일`, `번개 스타일`, `스텔스 가면`) are removed and replaced with the normal inventory material items `힘의 조각` and `마법의 가지`. Their names are copied verbatim in EUC-KR from the original title's `work.bar` item table. The emergency `부활의 기도문` party-wipe catalog is unchanged.

The catalog remains free/offline and uses the existing native purchase completion path; no direct save/inventory mutation is introduced.
