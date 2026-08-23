# WIPI Player Phase 8.22 — Inotia 1 corrected response framing + Inotia 2 presentation/CPU fast paths

Phase 8.22 is a full-repository release built directly from the Phase 8.21 tree.

## Inotia 1 — cash-shop error 2009 root cause

The Phase 8.21 field log still reports cash error `2009` / state `5` after the
27-byte command-1 experiment is completely consumed. Thumb disassembly of the
exact `010100D3 / PD005362` binary resolves the reason more precisely than the
earlier state-prime hypothesis.

The common receive dispatcher at guest `0x00117194` consumes **one byte from
every response frame before command-specific dispatch** and stores it through
the module GOT slot at `r10 + 0x470`.

- Command 0 only builds the authentic command-1 request when that byte is `1`.
- Command 1 only enters its real response parser when that byte is `1`.
- Command 1 with value `0` reaches the early error-2009 path at guest
  `0x00117258`, before the Phase 8.20/8.21 later validators.

Earlier local frames were malformed relative to that dispatcher contract:

- command-0 hello was `00 03 00`, with no common result byte;
- command-1 response was 27 bytes and began its payload with zero.

Phase 8.22 removes the Phase 8.17 forced command-0 branch and instead supplies
the original state machine with correctly framed responses:

- hello: `00 04 00 01` (length 4, command 0, common result/state 1)
- command 1: 28 bytes (`00 1c 01 01 ...`), where the extra `01` is the common
  result/state byte followed by the 24 bytes consumed by command 1's own parser.

No historical server is contacted. The existing Phase 8.20/8.21 guarded
legacy-validation bypasses remain in place in case the now-correct command-1
parser reaches them.

Diagnostic marker:

- `[PHASE8_22_INOTIA1_CASH_RESPONSE_STATE]`

The next useful result is either a new outbound cash command (command 2+) or a
new native cash error code different from 2009.

## Inotia 2 — repeated installation screen

The native initializer at guest `0x00144f48` cannot be skipped: the earlier
direct bypass caused the title's memory-error screen because the routine also
constructs required runtime resource tables.

Static disassembly isolates two calls from that initializer to the progress UI
renderer at guest `0x001449a0`. Phase 8.22 NOPs only those two presentation
call sites (`0x00144f86` and `0x00144fda`) under exact AID/PID/native-size and
byte guards. All resource initialization, cache expansion, pointer-table setup,
and completion logic still execute.

Marker:

- `[PHASE8_22_INOTIA2_INSTALL_UI_SUPPRESS]`

The obsolete installation/progress bar should no longer be presented even
though the required internal preparation still runs.

## Inotia 2 — animation, skills, and map-transition performance

Phase 8.21's 16,000-instruction slice did not improve the field symptoms, so
Phase 8.22 restores the better-observed 4,000-instruction title-specific slice.

More importantly, this phase targets two hot paths below the game itself:

1. **ARM interpreter memory access** — removes `RefCell` borrow checks from each
   guest memory callback, inlines the page lookup, uses safe page-crossing
   fallbacks, and uses unaligned native-width loads/stores on the normal
   within-page path.
2. **Web/iOS framebuffer presentation** — keeps RGB565 presentation frames as
   raw bytes, caches the browser 2D canvas context once, and expands RGB565
   directly to RGBA in one pass. The old route allocated/repacked framebuffer
   bytes into `Vec<u16>`, then `Vec<Color>`, then another RGBA `Vec` for every
   displayed frame.
3. **Release build optimization** — release codegen is constrained to one
   codegen unit and wasm-opt uses `-O4`.

Markers/source guards:

- `[PHASE8_22_INOTIA2_EXEC_QUANTUM]`
- `[PHASE8_22_ARM_MEMORY_FASTPATH]`
- `[PHASE8_22_RGB565_RAW_IMAGE]`
- `[PHASE8_22_WEB_RGB565_FASTPAINT]`

The pixel conversion is mathematically equivalent to the existing RGB565
conversion and is not title-specific; the Inotia 2 scheduler/install UI changes
remain exact-title-only.

## Compatibility scope

- Inotia 2: `AID 010100D5`, `PID PD007974` for title-specific patches.
- Inotia 1: `AID 010100D3`, `PID PD005362` for cash-shop compatibility.
- No new MapleStory or Heroes Lore 2 title-specific behavior is introduced.
