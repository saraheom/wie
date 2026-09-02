# WIPI Player Phase 8.82

Phase 8.82 keeps the Phase 8.80 generic virtual-JAR architecture, Phase 8.81 native freeze ring, and the working Chronos Wing 0xD9 compatibility path.

## Blade Master 3 database fix

The Phase 8.81 trace proved BM3 reaches LGT WIPI-C service `0x190 / OpenDatabase` and never returns. For exact title `000262F4 / PD109653`, database existence/open/close/available-storage now avoid the browser IndexedDB repository and use a per-emulator in-memory database mirror plus packaged-resource seeds. Missing READ opens return `-12` immediately; CREATE opens receive a normal guest database handle. New markers: `PHASE8_82_BM3_DB_OPEN_BEGIN`, `PHASE8_82_BM3_DB_SOURCE`, `PHASE8_82_BM3_DB_OPEN_RETURN`, `PHASE8_82_BM3_DB_CLOSE`, `PHASE8_82_BM3_DB_EXISTS`, and `PHASE8_82_BM3_DB_USAGE`.

## OZ Java hot-loop resolver

The Phase 8.81 recovered native ring showed OZ continuously entering and returning the same dynamic Java SVC (`0x4a856cb0`) rather than deadlocking. Phase 8.82 stores registration-time metadata for Java methods and synthetic class getters so runtime resolution does not depend on mutable guest method-table memory. Consecutive calls are counted and logged at useful thresholds via `PHASE8_82_OZ_JAVA_HOT_LOOP`, including resolved class/method/descriptor, caller LR, arguments, and call count.

TestFlight marketing version: **0.1.82**.
