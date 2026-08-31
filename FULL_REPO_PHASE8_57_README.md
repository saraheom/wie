# Phase 8.57 — OZ LGT AOT Null-Fault Localization

Phase 8.57 is based directly on Phase 8.56. The Phase 8.56 field log proves OZ now reaches `base/Koablo.startApp()` and fails with `net.wie.WieError: Invalid memory access; address: 0` only after WIPI display/annunciator startup. Previous class-link, InputMethodHandler, TextComponent, and OEMAppExecutor compatibility fixes remain intact.

This phase does not guess at the null pointer. It adds exception-only diagnostics at the two boundaries that previously lost the native guest context:

- `PHASE8_57_LGT_AOT_METHOD_FAULT` records the Java/AOT method name, descriptor, native entry, live PC/LR/R0-R3, raw arguments, and nearby ARM words when an AOT method exits with `InvalidMemoryAccess`.
- `PHASE8_57_ARM_SVC_FAULT` records the SVC category and live registers if the invalid-memory error originated inside a Rust SVC handler.

There is no continuous instruction trace and therefore no normal gameplay logging overhead. The next OZ launch should identify whether the null was produced by native game code or a specific LGT/WIPI service. Inotia1 cash-shop, EXP repair, save/revival behavior, and Inotia2 compatibility code are otherwise unchanged.
