# WIPI Player Phase 8.83

## Blade Master 3 — synchronous database intercept

Phase 8.82 proved that `OpenDatabase("setup.dat", 1, 1)` entered the BM3 session-memory branch but still fell through into Java resource/class-loader work before a handle was returned. Phase 8.83 moves the exact-title (`000262F4 / PD109653`) database interception to the top of `open_database()`, before packaged-resource lookup, Java class loading, or browser IndexedDB. BM3 opens now allocate a normal WIPI guest database handle synchronously from the per-emulator session mirror. Fresh missing databases are opened as empty local databases so `setup.dat` can initialize normally. Markers: `PHASE8_83_BM3_DB_SYNC_INTERCEPT` and `PHASE8_83_BM3_DB_SYNC_RETURN`.

## OZ — exception-loop callsite localization

Phase 8.82 resolved dynamic Java dispatcher `0x4a856cb0` to `java/lang/Exception.<get-initialized-class>()Ljava/lang/Class;`, repeatedly called from guest LR `0x00017b70`. Phase 8.83 captures the exact Thumb code window `0x00017b20..0x00017ba0`, full r0-r7/SP/LR/CPSR state, and a compact guest stack snapshot on the first loop hit. Sparse state milestones are retained for later iterations. Markers: `PHASE8_83_OZ_EXCEPTION_LOOP_CODE`, `PHASE8_83_OZ_EXCEPTION_LOOP_STACK`, and `PHASE8_83_OZ_EXCEPTION_LOOP_STATE`.

All Phase 8.80 generic virtual-JAR handling, Phase 8.81 persistent native ring, Phase 8.82 Java registration metadata, OZ `/kpool`/Rust-JAR fixes, BM3 graphics/media compatibility, and the working Chronos Wing `0xD9` handler are retained.

TestFlight marketing version: **0.1.83**.
