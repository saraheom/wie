# UI / Runtime Phase 8.30

## Inotia 1
- Respond to authentic resource-exchange command 89 with minimal success state.
- Bypass the remaining type-0xBA Network Mode state gate for the blessed seal,
  preserving the original item-effect continuation.
- Keep Phase 8.29 cash-shop pagination (12 + 6 records) unchanged.
- Do not auto-repair already-persisted character names without known originals
  or an uncorrupted save source.

## Inotia 2
- Preserve Phase 8.27 corrected LZMA acceleration.
- Preserve Phase 8.28 all-row RGB565 batch acceleration.
- Preserve the 4,000-instruction title-specific execution slice.
- Add only a lower one-shot NATIVE_LOOP diagnostic threshold (2,048 chunks) to
  isolate black-startup/main-menu CPU work; frame-stall logging remains disabled.
