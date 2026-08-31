# WIPI Player Phase 8.65 — OZ URLClassLoader.findResource Localization

Phase 8.65 keeps Phase 8.64 runtime behavior unchanged and adds focused diagnostics for the OZ black-screen path isolated in the 8.64 log. The last unmatched Java call was `java/net/URLClassLoader.findResource(Ljava/lang/String;)Ljava/net/URL;` after a nested `java/net/URL.getFile()` returned successfully.

New diagnostics:

- `PHASE8_65_OZ_FIND_RESOURCE_ENTRY`: raw loader/name-object pointers plus an 8-word snapshot of the requested resource-name object.
- `PHASE8_65_OZ_URL_GET_FILE_ENTRY`: raw URL object pointer plus an 8-word object snapshot.
- `PHASE8_65_OZ_URL_GET_FILE_RETURN`: returned Java String pointer from `URL.getFile()` plus an 8-word snapshot of that object.
- `PHASE8_65_OZ_FIND_RESOURCE_RETURN`: emitted only if `findResource()` returns; its absence confirms the same internal stall.
- Existing Phase 8.64 Java-call return logs now include the post-call R0 value.

No network-validation bypass or resource-loading behavior is changed in this phase.
