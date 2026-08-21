# Phase 8.1.2 — Inotia 1 future-proof save-length handling

This supersedes the Phase 8.1.1 growing-save patch.

## Why this is stronger

The earlier fixes were milestone-based:

- original behavior: effectively 320 bytes
- Phase 7.21: fixed records above 320, but had an arbitrary <=512 guard
- Phase 8.1.1: removed the 512-byte ceiling

Phase 8.1.2 removes size assumptions entirely for the affected KTF path.

For PID PD005362 / save0.dat / offset 0 / mode 0, every non-empty record now
returns:

    result = 320 - actual_record_length

Therefore Inotia's native wrapper:

    logical_length = 320 - result

always reconstructs the exact stored length, whether the save grows or shrinks.

Examples:

    stored 300 -> result +20  -> read 300
    stored 320 -> result   0  -> read 320
    stored 324 -> result  -4  -> read 324
    stored 544 -> result -224 -> read 544
    stored 4096 -> result -3776 -> read 4096

There is no gameplay-progress threshold left in this logic.

## Defensive invariant

`stream_read()` now also emits:

    [INOTIA1_LENGTH_MISMATCH]

if Inotia 1 ever requests a different byte count than the actual non-empty
save0.dat record length. It intentionally does not force extra bytes into the
guest buffer; that would risk memory corruption. The warning gives us an
immediate diagnostic if a different compatibility issue appears later.

## Scope

No behavior changes for other games, PIDs, databases, offsets, or modes.
The Phase 8.1 Inotia 2 diagnostics remain present.
