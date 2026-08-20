# Phase 7.20 — Inotia 1 post-Terry final-validation bypass experiment

This phase is intentionally narrow and title-specific.

## What Phase 7.19 proved

The pre-Terry and post-Terry Continue reads enter the same native call chain.

The captured stack resolves to:

- `0x11bd4a` → call to WIPI DB read wrapper `0x10becc`
- return `0x11bd4f`
- caller return `0x11d119`
- outer caller return `0x11b4cf`

Disassembly of `client.bin138532` then leads to the save validation routine at
`0x11ef60`.  Its final decision is:

```text
0x11f11a  CMP r4, r0
0x11f11c  BEQ 0x11f05a   ; validation accepted
0x11f11e  MOVS r0, #0    ; validation rejected
```

## Experiment

Only when all of these are true:

- PID is `PD005362`
- database is `save0.dat`
- Continue reads 320 bytes from offset 0
- the backing record is larger than 320 bytes
- the exact 18-byte validator signature matches

Phase 7.20 rewrites only the final conditional branch:

```text
9d d0  ->  9d e7
BEQ        B
```

The branch destination is unchanged.  All save decoding/transformation still
runs; only the final integrity rejection is bypassed.

No other title or binary revision is patched when the signature differs.

## Test

1. Start from a clean Slot 1.
2. Save before Terry and verify Continue works.
3. Load, talk to 경비병 테리, accept/progress the quest, save.
4. Return to Continue.
5. Check whether Slot 1 is now visible.
6. If visible, load it and verify:
   - character/name/level are correct
   - Terry quest state is present
   - movement/combat/menu still work
7. Exit and relaunch WIPI Player, then verify the same save again.
8. Export the global diagnostic log.

Expected marker for the post-Terry read:

`[INOTIA1_FIX] installed signature-guarded final-validation bypass ...`

If the slot appears and the quest state is correct, the remaining bug is the
integrity/footer path rather than persistence.  If the slot appears but state is
corrupt/incomplete, the next fix should restore the missing logical tail instead
of keeping this bypass.
