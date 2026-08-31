# UI Phase 8.56 — OZ WEC OEMAppExecutor Startup Compatibility

No user-facing UI change. Adds the LGT carrier-extension class `wec/OEMAppExecutor` required by OZ during imported-class linking. The class exposes the exact static/direct ABI observed in OZ's ARM ELF and logs any runtime invocation without pretending an external carrier application was launched.
