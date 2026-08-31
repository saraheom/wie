# Phase 8.59 — OZ LGT Class-Link / Vtable Localization

Phase 8.59 is based directly on Phase 8.58. It preserves the verified 64-bit J/D field continuation repair and all prior Inotia1/Inotia2/OZ compatibility changes.

The Phase 8.58 field continuation markers confirmed that `base/a` now links both `long` continuation words successfully, but the WebAssembly runtime then enters a repeating call-stack failure before the public-class link returns. Phase 8.59 adds error-localization markers around the remaining `base/a` class-link stages, each virtual member resolution, and the generated vtable hierarchy/read path. No speculative vtable behavior is changed in this phase.

Expected diagnostic markers include `PHASE8_59_LGT_LINK_CLASS_BEGIN`, `PHASE8_59_LGT_LINK_STAGE`, `PHASE8_59_LGT_VIRTUAL_RESOLVE_BEGIN`, `PHASE8_59_LGT_VTABLE_INDEX_BEGIN`, `PHASE8_59_LGT_VTABLE_HIERARCHY_BEGIN`, and matching completion markers.
