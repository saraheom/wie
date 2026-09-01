# Phase 8.70 — OZ accumulated chunked FileInputStream read

Phase 8.69 proved 64 KiB VFS reads succeed on iOS, but exposing the short read to Java caused ZipFile to parse a partial JAR and fail with `Could not find EOCD`.

Phase 8.70 keeps each underlying VFS transfer at <=64 KiB while accumulating internally until the Java caller's requested buffer is full or real EOF is reached.

Diagnostics:
- `PHASE8_70_OZ_FILE_READ_ACCUM_BEGIN`
- `PHASE8_70_OZ_FILE_READ_CHUNK_BEGIN`
- `PHASE8_70_OZ_FILE_READ_CHUNK_RETURN`
- `PHASE8_70_OZ_FILE_READ_ACCUM_RETURN`

Version: 0.1.70
