# WIPI Player Phase 8.72

## OZ JAR metadata cache fix

Phase 8.72 preserves the Phase 8.70 accumulated 64 KiB JAR read repair and the Phase 8.71 diagnostics, then fixes the repeated iOS VFS metadata hang by caching successful `.jar` sizes in `JvmRuntime`.

- Cache scope: successful metadata sizes for `.jar` paths only.
- Mutable save/config files are not cached and continue to query the filesystem.
- The cache is shared by cloned `JvmRuntime` instances during the app session.
- New OZ diagnostics: `PHASE8_72_OZ_METADATA_CACHE_MISS`, `...CACHE_STORE`, and `...CACHE_HIT`.
- Expected OZ behavior: the first `00026DBF.jar` metadata lookup stores 1,899,873 bytes; later lookups return immediately from the cache instead of awaiting `filesystem().size()`.

TestFlight marketing version: 0.1.72.
