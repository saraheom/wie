# UI / Runtime Phase 8.21

- Inotia 2: 16k title-scoped ARM execution slice for CPU-heavy animation/resource decoding.
- Inotia 2: shared per-launch persistent static-record cache (`db:` namespace) to avoid repeated IndexedDB reads.
- Inotia 1: bypass second obsolete command-1 data/session validation branch at guest 0x001174fe while preserving helper side effects.
- Preserves Phase 8.18 safe Inotia 2 installer/initializer; does not restore unsafe Phase 8.17 direct caller bypass.
