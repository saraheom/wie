# WIPI Player Phase 8.66 — OZ URLClassLoader Metadata-Hang Bypass

Phase 8.65 localized the OZ startup freeze to RustJava `java/net/URLClassLoader.findResource()`. The trace shows `URL.getFile()` returning successfully; the pinned RustJava source immediately next awaits `RuntimeContext::metadata(&file)`, and execution never returns.

Phase 8.66 adds an OZ-only diagnostic behavior change. For the exact `URLClassLoader.findResource(Ljava/lang/String;)Ljava/net/URL;` dispatcher call, WIE returns `null` instead of entering the hanging metadata lookup. No resource or class is fabricated. This allows the normal class-loader fallback / ClassNotFound path to continue and reveals whether this is an optional fallback lookup or a required packaged resource.

Markers:
- `PHASE8_66_OZ_FIND_RESOURCE_METADATA_BYPASS`
- `PHASE8_66_OZ_FIND_RESOURCE_METADATA_BYPASS_RETURN`

The bypass is gated by the existing OZ diagnostic flag, enabled only for AID `00026DBF`, PID `PD112525`.
