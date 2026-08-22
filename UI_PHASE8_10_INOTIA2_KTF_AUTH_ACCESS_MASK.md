# Phase 8.10 — Inotia 2 KTF legacy authentication access-mask compatibility

## What Phase 8.9.1 proved

Phase 8.9.1 fixed the Rust lifetime build failure and let the KTF Inotia 2 build
(`AID 010100D5`, `PID PD007974`) proceed well beyond the former
`MC_knlGetAccessLevel` fatal stub.  The game now reaches its own Com2uS/KTF
legacy authentication UI, where it reports error 1001.

The device log also confirms that the earlier Phase 8.9 `i_pack.dat` CREATE
rebuild/truncation behavior is active, so this is a later and independent gate.

## Why error 1001 occurs

Static analysis of this exact KTF `client.bin` identifies the authentication
routine at guest address `0x0012af8c`.

The routine:

1. calls the kernel interface function at offset `0x34`;
2. offset `0x34 / 4 = 13`, which is `MC_knlGetAccessLevel`;
3. computes `returned_access_level & 0xBC`;
4. requires that value to equal `0xBC`;
5. returns literal `0x03E9` (decimal 1001) when the access check fails.

Phase 8.9/8.9.1 returned `1`.  Therefore the guest remained alive, but
`1 & 0xBC == 0`, causing the exact error shown on screen.

If the access check succeeds, the same routine next calls kernel offset `0x74`
(`0x74 / 4 = 29`, `MC_knlGetSystemProperty`) for `PHONENUMBER`.  This build
explicitly treats a zero-length phone-number string as valid for this local
check, so WIE's current empty `PHONENUMBER` value does not need to be changed.

## Fix

For only the known Inotia 2 KTF identity:

- AID: `010100D5`
- PID: `PD007974`

`MC_knlGetAccessLevel` now returns the minimum permission mask the native game
requires:

    0xBC

Other titles retain WIE's prior behavior for this unimplemented call.  This
keeps the compatibility rule narrowly scoped and avoids changing MapleStory or
other games.

Runtime marker:

    [PHASE8_10_ACCESS] Inotia2 KTF legacy auth mask active: return=0xbc ...

## Expected test result

The specific `(오류번호:1001)` authentication dialog should no longer be
reached through this access-mask failure.  The game should take its normal
post-access-check branch and continue to the next startup/authentication state.
If a different legacy gate appears afterward, export the diagnostic log so the
next return code/state can be mapped without guessing.
