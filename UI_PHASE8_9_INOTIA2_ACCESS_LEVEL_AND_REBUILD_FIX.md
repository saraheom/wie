# Phase 8.9 — Inotia 2 access-level + i_pack rebuild fix

## What Phase 8.8 proved

Phase 8.8 fixed the previous startup/resource-table crash:

- game.dat SEEK_END now returns 40267;
- the game resource table is populated;
- resource 0x43 has a valid nonzero base;
- the old address-0 exception is gone;
- the title advances through its installation/initialization UI.

The new deterministic crash on both observed launches is:

    Unimplemented: 13: MC_knlGetAccessLevel

## Fix A — KTF kernel slot 13

WIE already exposes KTF kernel slot 13 as `MC_knlGetAccessLevel`, but the
current implementation is a fatal `Unimplemented` stub.

For PID `PD007974` only, Phase 8.9 returns the compatibility value `1`.
All other PIDs keep the previous behavior until a generic
ADF/security-provisioning implementation is established.

Marker:

    [PHASE8_9_ACCESS]

The first call also records the native LR/PC and r0-r3.

## Fix B — stop i_pack.dat growing on every failed launch

The device log also explains why initialization appears to download/rebuild
again. The persistent i_pack.dat sizes grow by exactly one packaged payload
generation:

    packaged template: 1,489,150 bytes
    after failed run:  2,978,245 bytes
    later run:         4,467,340 bytes

Mode 4 is the CREATE/rebuild path, but the previous code preserved an
existing persistent record whenever a packaged template also existed.
The guest then used SEEK_END and appended the newly rebuilt payload after
the stale generation.

For PID `PD007974`, `i_pack.dat`, mode 4 only, Phase 8.9 now truncates the
persistent record before the guest rebuilds it.

Marker:

    [PHASE8_9_IPACK_CREATE]

This does not delete the packaged `p/i_pack.dat` template and does not alter
normal mode-1 reads.

## Preserved compatibility work

- Phase 8.8 Inotia 2 KTF SEEK_SET / SEEK_CUR / SEEK_END semantics
- Phase 8.4 KTF packaged-resource P/ and p/ fallback
- Phase 8.1.2 Inotia 1 future-proof save-length behavior
- MapleStory/network behavior unchanged
- no ARM binary patch

## Test

A clean Inotia 2 saved-state reset is recommended once because the current
persistent i_pack.dat already contains several appended incomplete rebuild
generations and there is no user-created game save yet.

After installing Phase 8.9:

1. Erase only Inotia 2 saved state once.
2. Launch the original KTF Inotia 2.
3. Let initialization complete.
4. Check whether the title/new-game screen appears.
5. Exit and relaunch.
6. Verify that startup no longer accumulates another i_pack generation.

If another API boundary appears, export the log; it will be downstream of
the now-fixed resource initialization and access-level call.
