# WIPI Player Phase 8.41 — Inotia1 Continue Position Rescue

Phase 8.41 is a narrowly-scoped recovery build for the user's preserved 2026-08-26 Inotia1 progress backup. It keeps the Phase 8.37 normal gameplay/performance/12-item-catalog baseline and all Phase 8.40 emergency-prayer handoff and resurrection-context fixes. Inotia2 is unchanged.

## Why the JSON coordinate variants are retired

The Phase 8.40 field log from the original backup and the follow-up log from the x15-y17 JSON both enter the same guest `0x0013adcc` native loop with the same pathfinder arguments X=21/Y=27. The follow-up save0 hash changed, proving the edited JSON was restored, but the native arguments did not. Therefore the earlier assumption that those edited ciphertext bytes were the persisted X/Y field was wrong. Phase 8.41 does not edit the opaque save ciphertext.

## Exact native recovery

Static analysis resolves the stuck call to guest `0x0011d00e -> 0x0013ad04`. Immediately before it, guest `0x0011d008` copies R5 into R0. R5 is X and R1 is Y; the field trace shows X=21/Y=27. The same native routine clamps its search against a 16x18 active map, making that saved tuple out of bounds.

Phase 8.41 adds a hash-keyed hook at Thumb PC `0x0011d009`, replacing only the 16-bit `ADDS R0,R5,#0`. It always emulates the original instruction. It changes behavior only when all of the following are true:

- the exact callsite is reached;
- X=21 and Y=27;
- the live map dimensions are exactly 16x18.

Then it supplies X=15/Y=17 to the original pathfinder and, when the corresponding live state still exactly matches X=21/Y=27, repairs the packed coordinate global and selected-character fields at +0x23c/+0x240. This is intended to let Continue finish and let the game's own serializer create a clean save afterward.

Runtime diagnostic marker:

`PHASE8_41_INOTIA1_CONTINUE_POSITION_RESCUE`

## Test order

1. Keep the original 2026-08-26 00:12:47 backup untouched.
2. Install TestFlight 0.1.41 and confirm `PHASE8_41_RUNTIME_SENTINEL` in the log.
3. Restore the **original** backup, not any x/y-mutated recovery JSON.
4. Press Continue once.
5. If gameplay loads, immediately create a new backup before testing resurrection or the cash shop again.
6. Export the log. The rescue marker should report `input=(21,27)`, `width=Some(16)`, `height=Some(18)`, and `applied=true`.

If Continue still hangs, the marker's packed/character fields give the next exact state source without sacrificing the original backup.
