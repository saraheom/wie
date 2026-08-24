# WIPI Player Phase 8.25 — Inotia 1 async cash read wake + Inotia 2 RGB565 effect fast path

Phase 8.25 is a complete repository based on Phase 8.24.

## Inotia 1 — fix the second-entry 30-second timeout

The Phase 8.24 field log establishes a deterministic transport bug rather than
another guessed protocol field:

1. The title consumes the local command-2 and command-4 zero-byte transfer.
2. It registers `MC_netSetReadCB(fd=0, cb=0x0010c15d, param=0)`.
3. Later the title emits command 30 and Phase 8.24 queues a command-30 response.
4. No guest read occurs because the offline bridge discarded the callback
   registration instead of waking it when local data arrived.
5. Exactly about 30 seconds later the title closes the socket with cash error
   2014/state 5.

Phase 8.25 persists the latest Inotia-1 read callback as a one-shot waiter. When
a local response is queued, WIE schedules the original callback asynchronously
with the recovered ABI `(fd, status=0, param)`. The guest callback then invokes
legacy slot-32 RECV itself, preserving the title's original control flow. The
same wake helper closes the race where bytes are already pending when
`MC_netSetReadCB` is registered.

Markers:

- `[PHASE8_25_INOTIA1_CASH_READ_CALLBACK]`
- `[PHASE8_25_INOTIA1_CASH_READ_WAKE]`

The command-2/4 catalog transfer is still intentionally empty. Therefore the
first-entry equipment/background artifact may still remain. This phase fixes
the proven async transport deadlock so the command-30 response can be consumed
and the next authentic protocol request can be captured. It does not modify
`char.dat`, grant items, contact a historical server, or emulate billing.

## Inotia 2 — target the measured software-pixel hot loop

Phase 8.23 profiling tied the largest frame gaps to the repeated guest loop at
`0x00123f2a..0x00123f82`, with hot samples at `0x00123f82`. That loop performs
per-pixel 16-bit color/effect processing:

- unpack an RGB565 pixel;
- convert current R/G/B to 5-bit lookup indices;
- transform each channel through the same 32x32 byte LUT using a reference
  color;
- repack RGB565;
- repeat across the clipped row.

Phase 8.24's generic interpreter cleanup was behavior-preserving but did not
produce a visible improvement, so Phase 8.25 no longer makes broad scheduler or
storage experiments. For the exact Inotia 2 AID/PID/native length and original
hook bytes only, the first instruction of that row loop is replaced by a
private SVC. The host handler bulk-reads the 1024-byte LUT and the RGB565 row,
performs the same channel transformation in Rust, bulk-writes the row, restores
the register state expected at the natural loop exit, and resumes at
`0x00123f84`.

Safety gates:

- AID `010100D5`
- PID `PD007974`
- native length 608,192 bytes
- original instruction bytes `02 99` at guest `0x00123f2a`
- active pixel masks must be exactly RGB565: `F800 / 07E0 / 001F`
- clipped row width must be 1..4096

If any runtime invariant fails, the SVC is removed, original bytes `02 99` are
restored, and execution jumps back to the untouched guest loop for the rest of
the launch.

Marker:

- `[PHASE8_25_INOTIA2_RGB565_FASTPATH]`

The known-working 4,000-instruction Inotia 2 run slice remains unchanged. The
Phase 8.22 installer progress renderer remains hidden, while the required
initializer still executes to avoid the earlier `메모리에러` regression.

## Test focus

### Inotia 1

Enter the cash shop, exit if necessary, then enter it again in the same game
session. The key result is whether command 30 is followed by
`PHASE8_25_INOTIA1_CASH_READ_WAKE` and slot-32 RX of phase 5 instead of the
30-second error-2014 timeout. Any new outbound command after that is the next
protocol stage to implement.

### Inotia 2

Test ordinary movement, repeated skills, and several map transitions. The new
fast path specifically targets the measured software-effect row, so improvement
should be most visible when that effect routine was responsible for a hitch.
