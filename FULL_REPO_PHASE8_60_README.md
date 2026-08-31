# Phase 8.60 — OZ LGT Class-Initialization Re-entrancy Repair

Phase 8.60 is based directly on Phase 8.59 and preserves the verified LGT interface-method linker, J/D wide-field continuation repair, WIPI compatibility additions, AOT/SVC diagnostics, Inotia1 reward repair/cash catalog, and existing Inotia2 compatibility work.

Phase 8.59 proved that `base/a` field linking and all eight virtual-method resolutions complete successfully. Static disassembly then identified the subsequent stack exhaustion as recursive class initialization: generated initialized-class getter `0x18c0` invokes LGT `InitializeClass`; callback `0x197c` branches to class initializer `0x1904`; that initializer calls `0x18c0` again at `0x1920`.

The previous runtime left the class state unchanged until the callback returned, so every re-entry invoked `<clinit>` again. Phase 8.60 implements the LGT VM state transition expected by generated AOT code: state 4 means initialization in progress and state 5 means initialization complete. The runtime writes state 4 before calling the initializer, returns safely on same-class re-entry, promotes to state 5 only after successful callback completion, and restores the prior state if the callback fails.

Key markers: `PHASE8_60_LGT_CLASS_INIT_BEGIN`, `PHASE8_60_LGT_CLASS_INIT_REENTRANT`, `PHASE8_60_LGT_CLASS_INIT_COMPLETE`, and `PHASE8_60_LGT_CLASS_INIT_CALLBACK_ERROR`.
