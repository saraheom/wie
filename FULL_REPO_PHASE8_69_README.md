# Phase 8.69 — OZ chunked FileInputStream read

Phase 8.68 localized the OZ black-screen freeze to a Java `FileInputStream.read([BII)` request for the entire `00026DBF.jar` (1,899,873 bytes) in one operation. The file opens successfully; the oversized read never returns on iOS.

Phase 8.69 keeps the successful Phase 8.67 direct `binary.mod` bootstrap and changes the generic JVM file backend so one `File::read` transfers at most 64 KiB. Java stream reads are allowed to return fewer bytes than requested, so callers continue until EOF while avoiding the oversized WASM/IndexedDB transfer.

OZ diagnostics:
- `PHASE8_69_OZ_FILE_READ_BEGIN`
- `PHASE8_69_OZ_FILE_READ_RETURN`

Version: 0.1.69
