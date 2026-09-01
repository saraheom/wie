# Phase 8.73 — OZ classpath metadata negative-cache fix

Phase 8.73 keeps the Phase 8.70 accumulated 64 KiB JAR read fix and the Phase 8.72 positive JAR-size cache.

The 8.72 log proved that `wie.rustjar`, a synthetic RustJava classpath entry, can hang on its first asynchronous VFS `size()` query. Phase 8.73 changes the metadata cache to store `Option<FileSize>` and pre-seeds `RT_RUSTJAR`/`wie.rustjar` as a negative result (`None`). This lets `URLClassLoader.findResource()` fall through without touching the VFS for the synthetic missing entry.

New OZ diagnostics:
- `PHASE8_73_OZ_METADATA_CACHE_MISS`
- `PHASE8_73_OZ_METADATA_CACHE_STORE`
- `PHASE8_73_OZ_METADATA_CACHE_HIT`
- `PHASE8_73_OZ_METADATA_NEGATIVE_CACHE_HIT`

Writable/save files are not metadata-cached.
- `PHASE8_73_OZ_METADATA_NEGATIVE_CACHE_STORE` for other missing immutable classpath entries discovered at runtime.
