# Phase 8.58 — OZ LGT Wide-Field Linker Compatibility

Phase 8.58 is based directly on Phase 8.57. It preserves all prior Inotia1/Inotia2/OZ compatibility changes and adds generic LGT class-link support for 64-bit Java field word slots.

OZ `base/a` declares `long` (`J`) fields whose second 32-bit word is represented by a null/null metadata slot. The AOT code reads both output lookup entries. Phase 8.58 resolves the named `J`/`D` field to its starting word index and writes `word_index + 1` into the following continuation slot. Unexpected null slots remain fatal.

No Inotia1 cash-shop, reward-repair, save/revival, or Inotia2 gameplay behavior is changed.
