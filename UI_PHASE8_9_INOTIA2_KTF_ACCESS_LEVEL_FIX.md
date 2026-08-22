# Phase 8.9 — Inotia 2 KTF MC_knlGetAccessLevel compatibility

## Phase 8.8 result

Phase 8.8 fixed the previous resource-init crash. The new device log shows:

- no `Invalid memory access` exception;
- resource 0x43 is initialized (`kind=2`, non-zero base);
- initialization progresses much farther;
- both observed crashes terminate for the exact same reason:

    Unimplemented: 13: MC_knlGetAccessLevel

The second launch reproduces the same API failure.

## Why the apparent download repeats

The failed initialization is being persisted. `i_pack.dat` grows across the
failed runs, so this is not an IndexedDB flush failure. The retry is re-running
unfinished game initialization because the guest thread is aborted before it
can reach its normal completion state.

Do not change or truncate the database path in this phase.

## Fix

KTF kernel interface slot 13 already exists, but WIE routed it to a fatal
`Unimplemented` stub.

For PID `PD007974` only, Phase 8.9 returns:

    MC_knlGetAccessLevel() -> 1

which represents the WIPI CP (content-provider) security class used for this
commercial KTF title.

All other titles retain the existing behavior until a generic KTF
security-metadata implementation is established.

The compatibility marker is emitted once:

    [PHASE8_9] Inotia2 MC_knlGetAccessLevel compatibility active: level=1 (CP)

## Preserved behavior

- Phase 8.8 KTF SEEK_SET / SEEK_CUR / SEEK_END fix for Inotia 2
- Phase 8.4 KTF P/ / p/ packaged-resource fallback
- Phase 8.1.2 Inotia 1 future-proof save-length behavior
- MapleStory/network behavior is untouched

## Test recommendation

Because the current Inotia 2 database contains repeated *incomplete*
pre-game initialization payloads and no user-created game progress yet, a
clean test is preferable:

1. Install Phase 8.9.
2. Erase Inotia 2 saved state once.
3. Launch the original KTF Inotia 2.
4. Allow initialization to finish.
5. Check whether the title/new-game screen appears.
6. Fully exit and relaunch once to verify initialization is not repeated.

If the title screen appears but a later API fails, export that log; it is a
new post-initialization compatibility boundary.
