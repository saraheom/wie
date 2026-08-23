# UI / Runtime Phase 8.23

## Inotia 1
- Preserve Phase 8.22 corrected common-state framing.
- When authentic command 5 is emitted, queue a minimal command-2 transfer-start
  frame with success state and zero data length.
- Do not fabricate catalog contents or purchase/inventory/save writes yet.

## Inotia 2
- Preserve install-progress UI suppression and the required native initializer.
- Keep 4,000-instruction gameplay scheduling.
- Add title-scoped lower-threshold native-loop diagnostics.
- Add frame-gap and presentation-cost diagnostics around `MC_grpFlushLcd`.
