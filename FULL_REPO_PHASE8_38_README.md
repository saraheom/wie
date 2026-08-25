# WIPI Player Phase 8.38 — Inotia1 Party-Wipe Prayer Recovery

Phase 8.38 is intentionally based on the field-validated Phase 8.37 baseline. It does not change normal gameplay scheduling, the normal 12-item cash catalog, the main-character name repair, save seek behavior, or Inotia2.

## Backup evidence

The supplied working and post-wipe backups contain the same `char.dat1`, `map.dat1`, `mon.dat1`, `pattern.dat1`, `save1.dat1`, and `tile.dat1`. Only `prefs1` and `save0.dat1` differ. The post-wipe `save0.dat1` is 1904 bytes versus 1872 bytes in the working reference. Because `save0` is opaque/high-entropy, Phase 8.38 does not splice, truncate, or rewrite backup ciphertext.

## Recovered original game behavior

Static analysis of the original Inotia1 assets identifies `부활의 기도문` as item ID 537 (`0x219`). The original help text states that it can revive the whole party on the spot without the normal total-wipe penalty; the ordinary total-wipe path applies a 5% money/EXP penalty.

The native party-wipe input handler at guest `0x0014a444` uses `r10+0x25c` as its outer state. When the user selects prayer revival:

- if item `0x219` exists, the title enters state 13 and the native update path consumes the prayer;
- if item `0x219` is missing, the title enters state 14 and offers the network purchase path;
- pressing CLEAR in native state 14 performs exactly `state = 11` and `selection = 0`.

Phase 8.38 mirrors only that proven CLEAR transition when leaving the emergency cash overlay.

## Runtime behavior

At outbound cash command 5 only, the bridge reads the three recovered death-UI globals. If the outer state is exactly 14, it marks the session as an emergency prayer purchase and serves a one-record free catalog containing only `부활의 기도문`. Normal cash sessions continue to receive the unchanged Phase 8.37 12-record catalog.

If command 123 closes an emergency session while the outer state is still 14, the bridge writes state 11 and selection 0, then sends the already validated `[00 04 7b 01]` native cleanup response. This does not force resurrection or penalty logic; it returns control to the title's original death prompt.

There is no per-frame, repaint-time, or per-key party-wipe tracing in this phase.

## Existing broken backup

Phase 8.38 primarily fixes the transition that creates the stranded emergency state. It deliberately does not guess at opaque `save0.dat1` bytes. Importing the already-broken backup is still useful as a recovery test; if Continue still crashes, export that Phase 8.38 crash log so the stale persisted state can be isolated without damaging valid save data.
