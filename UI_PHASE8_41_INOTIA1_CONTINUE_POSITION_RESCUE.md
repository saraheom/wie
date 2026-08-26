# Phase 8.41 test notes

Use the untouched original progress backup from 2026-08-26 00:12:47. Do not use the earlier x15-y17/fallback JSON files; the new field log proved they modified the wrong opaque-save bytes.

Expected runtime markers:
- `PHASE8_41_RUNTIME_SENTINEL`
- `PHASE8_41_INOTIA1_CONTINUE_POSITION_RESCUE`

Expected rescue marker for the damaged backup:
- `input=(21,27)`
- `width=Some(16)`
- `height=Some(18)`
- `applied=true`

If Continue loads, immediately make a new in-app backup. If it does not, export the log without trying the old fallback JSON variants.
