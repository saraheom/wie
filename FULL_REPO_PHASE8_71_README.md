# Phase 8.71 — OZ late URLClassLoader localization

Preserves Phase 8.70 accumulated 64 KiB internal VFS reads. Adds focused diagnostics for the later `URLClassLoader.findResource()` freeze reached after `startApp()`.

New markers:
- `PHASE8_71_OZ_FIND_RESOURCE_ENTRY` with decoded requested resource/class string
- `PHASE8_71_OZ_URL_GET_FILE_RETURN` with decoded URL file/path
- `PHASE8_71_OZ_METADATA_ENTRY`
- `PHASE8_71_OZ_METADATA_SIZE_BEGIN` / `PHASE8_71_OZ_METADATA_SIZE_RETURN`
- `PHASE8_71_OZ_METADATA_RETURN`

Version: 0.1.71
