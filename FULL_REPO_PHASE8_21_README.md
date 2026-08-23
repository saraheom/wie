# WIPI Player Phase 8.21 — Inotia 2 CPU/installed-record cache + Inotia 1 command-1 data validation

Phase 8.21 is a full-repository release built on Phase 8.20.

## Inotia 1 — offline cash shop

The Phase 8.20 field log proved that:

- the original command-1 request is transmitted through the offline bridge;
- the complete 27-byte local response is consumed;
- the first legacy validation branch at guest `0x00117418` is already bypassed;
- the title still closes the socket with cash error `2009` and state `5`.

Static analysis of the same command-1 handler resolves a second direct error-2009 branch at guest `0x001174fe`, immediately after helper `0x0011d158`. The helper advances packet state and invokes a historical carrier/session integrity check. Phase 8.21 preserves that helper call and side effects, but NOPs only its `beq error_2009` branch for exact AID/PID/native-size match.

Marker:

`[PHASE8_21_INOTIA1_CASH_CMD1_DATA_VALIDATION_BYPASS]`

The next objective remains to capture the original command 2/3/catalog request before synthesizing free purchase results.

## Inotia 2 — performance

Phase 8.20 proved the packaged-resource cache is now genuinely shared, so repeated archive/JVM loads are no longer the dominant remaining stutter source. The field log still shows very long guest-native CPU loops while resource/image data is processed, and the user reports hitching in title animation, skills, and map transitions.

Phase 8.21 therefore:

1. Raises only `010100D5 / PD007974` from a 4,000 to a 16,000 guest-instruction execution slice. This reduces cooperative Rust/WASM yield traffic by 75% during CPU-bound native decoding/rendering while leaving every other title at the default slice.
2. Adds a second per-launch cache for the *persistent installed* static records (`i_pack.dat`, `eventdata.dat`, `filetext.dat`, `i_mapfeature.dat`, `i_tile.dat`). The cache is stored in the same shared KTF context Arc, uses a separate `db:` key namespace, is invalidated on CREATE, and is refreshed after write/close. Repeated normal opens can therefore avoid IndexedDB `exists/open/get` traffic.

Markers:

- `[PHASE8_21_INOTIA2_EXEC_QUANTUM]`
- `[PHASE8_21_INOTIA2_DB_CACHE]`

### Repeated installation screen

The installer/initializer remains enabled intentionally. Earlier direct bypassing of the caller removed the progress screen but also skipped required guest resource-table initialization and caused `메모리에러`. The current log proves the title still deliberately enters CREATE/rebuild even when the persistent generated caches already have their valid base+8 installed lengths. Phase 8.21 accelerates that required pass but does not reintroduce the unsafe caller bypass.

The next safe route for eliminating the visible install pass is to make the title's own verifier reconstruct/load the required runtime tables from the persistent cache, rather than bypassing the initializer wholesale.

## Scope

All new performance and protocol compatibility paths remain restricted to the exact Inotia titles. No intentional MapleStory or Heroes Lore 2 behavior changes are included.
