# Phase 7.21 — Inotia 1 KTF record-length seek-return fix

This phase removes the unsafe Phase 7.20 native validation bypass entirely.
It is based on the non-mutating Phase 7.19 database/ARM trace.

## Why Phase 7.20 crashed

The bypass forced Inotia's validator to return success even though its decoded
state was inconsistent. The game then continued with invalid state and hit an
unmapped address. Therefore the validator itself is protective, not the root
cause.

## New finding

`client.bin138532` contains a KTF DB wrapper that does:

    ret = slot4(handle, 0, 0);
    logical_len = 0x140 - ret;

WIE's `select_record_ktf` always returned 0, so the native loader always derived
320 bytes.

That is correct before Terry:

    stored len = 320
    slot4 ret  = 0
    load len   = 320

But it is wrong after Terry:

    stored len = 324
    old ret    = 0
    old load   = 320   <-- 4 bytes omitted

For Inotia 1 `save0.dat` only, Phase 7.21 returns the signed delta from 320 when
an existing record is larger:

    320 bytes ->  0
    324 bytes -> -4
    328 bytes -> -8

The native wrapper therefore reconstructs the actual logical record size.

## Scope

Guarded to:

- PID `PD005362`
- DB `save0.dat`
- offset 0
- mode 0
- existing record length 321..512 bytes

New/empty saves and every other title/database keep the prior return value.

No guest ARM instructions are patched.

## Expected log after Terry

    [INOTIA1_SEEK_FIX] db=save0.dat len=324 baseline=320 return=-4 expected_read_len=324
    [INOTIA1_SAVE] READ db=save0.dat ... request=324 returned=324 ...

If Continue then shows Slot 1 and it loads correctly, this identifies the KTF
slot-4 return-value semantics as the underlying compatibility bug.
