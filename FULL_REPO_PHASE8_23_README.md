# WIPI Player Phase 8.23 — Inotia 1 command-5 transfer probe + Inotia 2 stall profiler

Phase 8.23 is a full-repository release built directly from Phase 8.22.

## Inotia 1 — cash shop now reaches command 5

Phase 8.22 corrected the common response-state framing. Field testing proves the
original command-0/command-1 handshake now succeeds far enough for the game to
emit its next authentic request:

`00 0c 05 00 00 00 00 00 00 00 01 01`

The visible `연결 실패` message in Phase 8.22 is therefore no longer an
initial network/authentication failure. The offline bridge simply had no
response queued for command 5 and returned `M_E_WOULDBLOCK` while the game
waited for the next server stage.

Static analysis of the exact `010100D3 / PD005362` native image resolves the
next server-to-client stage as command 2. Its handler requires common
result/state 1, then consumes a big-endian u16 data length and exactly that many
bytes. Phase 8.23 queues the smallest structurally valid command-2 transfer
start after authentic command 5:

`00 06 02 01 00 00`

This intentionally declares zero catalog bytes. It is a protocol probe, not a
fabricated item catalog. No command-3/4 continuation, purchase result, currency
change, inventory write, or save mutation is synthesized yet. The next field
log should reveal whether the original client advances to another transfer
stage, sends another outbound request, or reports a new native status.

Marker:

- `[PHASE8_23_INOTIA1_CASH_CMD5_STAGE2]`

## Inotia 2 — installation UI and startup

Phase 8.22's progress-render suppression is retained. The visible installation
bar is gone, while the required initializer still runs underneath; therefore a
black interval before the normal loading/initialization screens is expected.
The latest field log confirms the hidden initializer still performs very large
native decode loops. Skipping that routine is not safe because the earlier
whole-call bypass caused the game's `메모리에러` path.

## Inotia 2 — map/skill lag diagnosis

Phase 8.22 removed several host-side framebuffer/interpreter costs, but the
remaining subjective hitch did not disappear. The latest field log only
contains deep native-loop diagnostics during startup, not during the tested
map/skill interval, so it is not yet justified to label the remaining hitch as
intrinsic game behavior.

Phase 8.23 adds diagnostics rather than another speculative scheduler change:

1. The exact Inotia 2 title remains at the known-good 4,000-instruction slice.
2. Its one-shot `NATIVE_LOOP` diagnostic threshold is lowered from 16,384 to
   1,024 consecutive slices (~4.1M guest instructions) for this title only.
   Execution semantics are unchanged; only logging becomes more sensitive.
3. `MC_grpFlushLcd` records frame gaps between 40 ms and 2,000 ms and logs the
   guest PC/LR at the presentation boundary.
4. Framebuffer-copy and host paint costs are logged separately when either
   reaches at least 8 ms.

Markers:

- `[PHASE8_23_INOTIA2_NATIVE_STALL_PROBE]`
- `[PHASE8_23_INOTIA2_FRAME_STALL]`
- `[PHASE8_23_INOTIA2_PRESENT_COST]`

These traces should separate three cases on the next test:

- long guest-native loop -> optimize/replace the responsible decoder/game path;
- cheap guest interval but expensive presentation -> optimize framebuffer/paint;
- neither -> the pause is likely intentional game pacing/timer behavior.

## Scope

All new protocol behavior is restricted to Inotia 1 (`010100D3 / PD005362`).
The lower native-loop diagnostic threshold and frame-stall diagnostics are
restricted to Inotia 2 (`010100D5 / PD007974`). No new MapleStory/Heroes Lore
compatibility behavior is intentionally changed.
