# WIPI Player Phase 8.81 — Native Freeze Ring + Deep Persistent Breadcrumbs

Phase 8.81 preserves Phase 8.80 generic virtual-JAR existence/metadata/read handling and all existing OZ, Blade Master 3, and Chronos Wing compatibility changes.

## High-information freeze diagnostics
- Adds a force-close-safe, synchronous **last-64 native boundary ring** in `debug_log.ts`.
- Captures Java entry/return, target WIPI-C entry/return, virtual-JAR/file metadata boundaries, OZ network/classpath boundaries, presentation/update boundaries, and frame boundaries.
- The next app launch dumps the previous ring using `PHASE8_81_RECOVERED_NATIVE_RING_BEGIN`, `PHASE8_81_RECOVERED_NATIVE_RING`, and `PHASE8_81_RECOVERED_NATIVE_RING_END`.
- The exported log also includes the current live ring section.

## OZ + Blade Master 3 WIPI-C decoding
- Adds `PHASE8_81_TARGET_WIPIC_ENTRY/RETURN` for exact OZ (`00026DBF/PD112525`) and Blade Master 3 (`000262F4/PD109653`).
- Each entry includes the resolved `WIPICSvcId` name, numeric ID, PC/LR/SP, and r0-r7.
- This allows a force-close to identify an entry without return directly (for example `0x190 / OpenDatabase`).

TestFlight marketing version: **0.1.81**.
