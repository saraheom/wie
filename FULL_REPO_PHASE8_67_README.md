# Phase 8.67 — OZ direct binary.mod bootstrap

Phase 8.65 localized the black-screen hang to `URLClassLoader.findResource()` after `URL.getFile()`. Phase 8.66 showed that returning null is not valid for this first lookup because LGT bootstrap immediately unwraps `getResourceAsStream("binary.mod")`.

For OZ only (`00026DBF` / `PD112525`), Phase 8.67 reads the already-mounted `00026DBF.jar` through WIE's virtual filesystem, extracts `binary.mod` from the ZIP in memory, and passes those real bytes to `load_native()`. The Phase 8.66 return-null bypass is removed, so later URLClassLoader resource semantics are unchanged.

Marker: `PHASE8_67_OZ_DIRECT_BINARY_MOD`.
