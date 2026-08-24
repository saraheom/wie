# WIPI Player Phase 8.33.1

Phase 8.33.1 is a TestFlight workflow-only correction to Phase 8.33.

## What failed in Phase 8.33

The GitHub Actions job stopped in the pre-build `Force clean` sanity step before Rust compilation. The workflow still asserted the obsolete Phase 8.32 three-page Inotia1 catalog array sizes (`118 / 96 / 118`). Phase 8.33 intentionally changed the catalog to two nine-item frames (`160 / 165`), so `grep` returned exit status 1 under `set -euo pipefail`.

## Fix

- Update both workflow sanity blocks to require Phase 8.33 page-0 `[u8; 160]` and page-1 `[u8; 165]` arrays.
- Require that the obsolete third `PAGE2_FREE` catalog is absent.
- Rename the workflow labels/messages from Phase 8.32 to Phase 8.33.
- No emulator/runtime behavior was changed from Phase 8.33.

The Phase 8.33 two-page Inotia1 cash catalog, heap-name recovery probe, validated special-item paths, and all retained Inotia2 behavior are otherwise unchanged.
