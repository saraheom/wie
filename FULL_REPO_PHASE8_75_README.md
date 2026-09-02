# WIPI Player Phase 8.75

## OZ virtual-JAR in-memory read fix

Phase 8.74 proved that reducing persistent VFS reads to 16 KiB did not eliminate intermittent iOS stalls. Inspection of `FilesystemOverlay` showed that normal reads prefer the persistent platform filesystem whenever a materialized copy exists, even when the same immutable imported JAR remains mounted in the in-memory virtual layer.

Phase 8.75 keeps the Phase 8.70 accumulated-read fallback and Phase 8.73 positive/negative metadata caches, but for exact OZ (`AID=00026DBF`, `PID=PD112525`) read-only access to `00026DBF.jar`, `FileImpl::read()` now serves bytes directly from `FilesystemOverlay::virtual_files` through `read_virtual()`. This avoids IndexedDB/platform filesystem awaits for the packaged JAR while leaving saves/configuration files and normal overlay shadowing semantics unchanged.

Expected diagnostics:

- `PHASE8_75_OZ_VIRTUAL_JAR_READ_BEGIN`
- `PHASE8_75_OZ_VIRTUAL_JAR_READ_RETURN`
- `PHASE8_75_OZ_VIRTUAL_JAR_READ_FALLBACK` only if the mounted virtual JAR is unexpectedly absent.

TestFlight marketing version: 0.1.75.
