# Phase 8.1.1 — Inotia 1 growing-save length fix

This patch preserves the Phase 8.1 Inotia 2 diagnostics and extends the
working Inotia 1 KTF record-length fix.

## Regression found after longer gameplay

The Phase 7.21 fix was guarded to save0.dat lengths 321..512 bytes.
That upper bound was only a safety guard, not part of the KTF ABI.

After additional Inotia 1 progress, save0.dat grew to 544 bytes.
Because 544 exceeded the guard, `select_record_ktf()` fell back to returning
0. Inotia's native wrapper therefore derived:

    0x140 - 0 = 320 bytes

and Continue read only 320 of the 544-byte record, leaving 224 bytes unread.
The slot was then rejected again.

## Fix

For PID PD005362 / save0.dat / offset 0 / mode 0:

- 320 bytes -> return 0
- any length > 320 -> return `320 - record_len`

The arbitrary `<= 512` cap is removed.

For the observed 544-byte save:

    return = 320 - 544 = -224
    native read length = 320 - (-224) = 544

All other titles, databases, offsets, and modes retain the previous behavior.

## Expected log

    [INOTIA1_SEEK_FIX] db=save0.dat len=544 baseline=320 return=-224 expected_read_len=544
    [INOTIA1_SAVE] READ db=save0.dat ... request=544 returned=544 ... remaining=0

The existing backup should be usable; no save reset is required.
