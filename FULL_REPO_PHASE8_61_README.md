# Phase 8.61 — OZ LGT Exception ABI Alignment

Phase 8.61 is based directly on the tested Phase 8.60 full repository. The 8.60 run proved that `base/a` class linking, vtable resolution, and class-initialization re-entrancy now progress beyond the prior failure. The next blocker occurs after a real Java `NullPointerException`, while the LGT runtime is processing the generated exception/type-check path.

This phase aligns two LGT Java imports with the current upstream WIE ABI: import `0x12` is treated as class assignability (`source class -> target class name`) rather than matching a pending exception object, and import `0x21` throws the supplied exception object without popping the compiler exception frame.

The assignability path also emits bounded diagnostic information for the three guest arguments. If OZ still supplies a malformed class-name pointer, the next log will contain the direct pointer values, nearby words, first bytes, and a possible one-level indirect string candidate before stopping. No invalid class name is silently accepted.

All existing Inotia1 reward/save/cash-shop repairs, Inotia2 compatibility/performance work, WIPI UI work, and the Phase 8.60 class-init guard are preserved.
