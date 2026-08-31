# Phase 8.53 — LGT Imported-Interface Linking + Startup Diagnostics

Phase 8.53 is based directly on the Phase 8.52 TestFlight CI-fix full repository. Inotia1 EXP, cash-shop, save/revival behavior and the existing Inotia2 compatibility work are preserved.

## OZ startup repair

The normalized OZ LGT build (`AID 00026DBF`, `PID PD112525`) reaches native initialization and then links 44 imported Java classes. Its `org/kwis/msf/io/Socket` import declares two interface methods: `getInputStream()Ljava/io/InputStream;` and `getOutputStream()Ljava/io/OutputStream;`. Phase 8.52 deliberately aborted whenever `interface_method_count != 0`. Phase 8.53 replaces that fatal guard with generic interface-method index resolution, matching the current upstream LGT ABI implementation.

## Diagnostics

New concise markers make later LGT failures visible in exported logs:
- `PHASE8_53_LGT_NATIVE_ENTRY`
- `PHASE8_53_LGT_INIT_STRUCT`
- `PHASE8_53_LGT_INTERFACE_LINK`
- `PHASE8_53_LGT_INTERFACE_METHOD`
- `PHASE8_53_LGT_UNSUPPORTED_JAVA_IMPORT`
- `PHASE8_53_LGT_UNSUPPORTED_IMPORT`
- `PHASE8_53_LGT_INITIALIZER_ERROR`
- `PHASE8_53_LGT_STARTUP_ERROR`
- `PHASE8_53_LGT_INITIALIZER_COMPLETE` / `PHASE8_53_LGT_STARTUP_COMPLETE`

This phase does not fake unknown Java imports such as 0x38/0x40. If OZ reaches one, the exact import will now be logged so its ABI can be implemented safely in the next pass.

## Inotia1

Unchanged from Phase 8.52: global 64-bit monster reward overflow repair and the 11-record cash catalog, with both enhancement scrolls and both material items at quantity 10.
