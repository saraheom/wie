# WIPI Player Phase 8.74

OZ iOS VFS-read hardening.

- Preserves Phase 8.70 full Java-visible accumulated reads.
- Reduces each underlying VFS/IndexedDB read from 64 KiB to 16 KiB.
- Preserves Phase 8.73 positive and negative classpath metadata caches.
- OZ chunk diagnostics are emitted only at milestones to reduce log volume.
- TestFlight marketing version: 0.1.74.
