# WIPI Player Phase 8.64 — OZ LGT Java Dispatch Localization

Phase 8.64 keeps all Phase 8.63 runtime behavior unchanged. For OZ startup it resolves each dynamic LGT Java dispatcher id into class, method, and descriptor and logs `PHASE8_64_OZ_JAVA_CALL_ENTRY` / `PHASE8_64_OZ_JAVA_CALL_RETURN`. The previous all-SVC boundary trace is downgraded to debug to avoid log flooding. The final Java CALL_ENTRY without a matching CALL_RETURN identifies the method that owns the black-screen hang.
