# UI / Runtime Phase 8.24

## Inotia 1
- Complete the Phase 8.23 zero-byte command-2 transfer with an empty command-4 finalizer.
- Treat observed command 123 as a one-way local session reset/cancel.
- Answer observed command 30 with a minimal success/zero-record response so a second cash-shop entry does not remain waiting forever.
- Do not synthesize item records, free purchases, cash currency, or inventory mutations yet.

## Inotia 2
- Keep the 4,000-instruction title-scoped execution slice.
- Remove Phase 8.23 high-volume frame/native-loop profiling from normal gameplay.
- Reuse the already-read guest PC in the ARM32 interpreter's SVC-vector test to reduce per-instruction interpreter overhead.
- Preserve the Phase 8.22 RGB565/Web fast-paint path.
- Keep the visible installer UI suppressed while retaining required initialization.
