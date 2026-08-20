# Phase 7.16 — Inotia 1 Full Save Record Replacement

## New evidence from the Phase 7.15 test

After the first quest interaction, `save0.dat1` is still missing from the in-game Slot 1 list even though the host log confirms successful persistence.

The decisive sequence is:

- an existing `save0.dat` record is reopened at 328 bytes;
- Inotia 1 then performs a stream write beginning at offset 0;
- the new write is 324 bytes long;
- Phase 7.15 keeps `buffer_len = max(old_len, new_end)`, so the record remains 328 bytes;
- the final four bytes therefore come from the previous record image rather than the new save.

That creates a mixed/stale-tail save image. The repository write succeeds, but Inotia can reject the slot during its own validation.

## Fix

For Inotia 1 (`PD005362`) save databases only, a stream write that begins at offset 0 is treated as a complete record replacement:

- overwrite the new bytes as before;
- set the logical record length to the end of the new write even when it shrinks;
- persist only that exact replacement image;
- emit `[INOTIA1_SAVE] full-record replace ...` diagnostics.

The behavior remains unchanged for other titles because KTF games may use offset writes inside shared/multi-slot records where preserving the untouched tail is required.

## Expected diagnostic after the Terry quest save

A successful replacement should now look like:

`[INOTIA1_SAVE] full-record replace db=save0.dat old_bytes=328 new_bytes=324`

followed by a host commit whose byte count matches the replacement length rather than retaining the stale tail.
