# UI Phase 8.54 — OZ LGT Input Method Compatibility

No user-facing UI changes. The runtime now exposes the standard WIPI `InputMethodHandler.getCurrentMode()I` method required by OZ and keeps its value synchronized with `setCurrentMode(int)`. Existing LGT startup diagnostics remain active so the next unsupported dependency, if any, is named directly in the exported log.
