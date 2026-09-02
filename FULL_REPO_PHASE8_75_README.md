# WIPI Player Phase 8.75

OZ LGT compatibility phase.

- Retains Phase 8.74 16 KiB accumulated VFS reads.
- Retains positive metadata caching for the real application JAR.
- Adds an unconditional metadata short-circuit for the synthetic `wie.rustjar` classpath entry before any VFS `size()` await.
- Diagnostic marker: `PHASE8_75_OZ_RUSTJAR_METADATA_SHORT_CIRCUIT`.
- TestFlight marketing version: 0.1.75.
