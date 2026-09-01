# UI Phase 8.66 — OZ URLClassLoader metadata bypass

No UI changes. OZ-only runtime experiment to bypass the confirmed `RuntimeContext::metadata()` hang in `URLClassLoader.findResource()` by conservatively returning null.
