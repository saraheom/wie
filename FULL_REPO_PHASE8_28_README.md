# WIPI Player Phase 8.28 — Inotia 1 network-item use/catalog + Inotia 2 all-effects batching

Phase 8.28 is a full-repository release built on Phase 8.27.

## Inotia 1 (010100D3 / PD005362)

### `자원 교환권` in single-player

The original client describes `자원 교환권` as exchanging for 10 network resources, but its use path checks that the global network state is exactly 2. Static analysis of the exact 431,008-byte native image resolves that check at guest `0x0015032e`. If the check fails, the same routine writes the client's error code 2001 and returns through its generic network failure UI.

Phase 8.28 changes only that exact `BEQ` to the existing valid-use continuation for this exact title/binary. It does **not** globally fake Internet mode and does not directly edit inventory/save data. The game's original item-use path remains responsible for consuming the ticket and applying its resource effect.

Marker:

`[PHASE8_28_INOTIA1_NETWORK_USE_GATE]`

The same error-2001 gate is also the strongest static match for the residual first-entry cash-shop popup that occurs after the catalog has already arrived, so this patch may remove that popup as well.

### Expanded offline cash catalog

Phase 8.27's 14-item catalog is expanded to 18 entries. The following four high-confidence network-only equipment definitions are appended using the exact unique EUC-KR names stored in the original `work.bar`:

- `흑기사의 투구`
- `레게 스타일`
- `번개 스타일`
- `스텔스 가면`

They remain price 0. The guest resolves the original item icon, stats, restrictions, inventory insertion, and save serialization through its own code.

Marker:

`[PHASE8_28_INOTIA1_CASH_CATALOG] ... records=18 ...`

## Inotia 2 (010100D5 / PD007974)

### Graphics-all-on performance

Phase 8.25 proved the RGB565 host hook materially improves ordinary gameplay. Its remaining weakness is that it handles only one clipped row per SVC. When shadow/weather/critical effects are enabled, the title executes far more effect rows, multiplying SVC dispatch, guest mask/LUT reads, and host buffer-allocation overhead.

Phase 8.28 keeps the same exact RGB565 transform but batches **all remaining rows of one effect rectangle in a single host call**. It also:

- caches the 1,024-byte 32x32 transform LUT for the launch until its guest pointer changes;
- reuses one host pixel buffer;
- preserves padded guest row strides;
- emulates the original outer-loop row counter/register state;
- resumes at the original function epilogue after the final row;
- keeps exact AID/PID/native-image/original-instruction install guards;
- keeps runtime RGB565 mask, dimensions, stride, and memory guards.

Markers:

- `[PHASE8_28_INOTIA2_RGB565_BATCH] installed ...`
- `[PHASE8_28_INOTIA2_RGB565_BATCH] first batch accelerated ...`

If any invariant is unexpected before pixel modification, the original guest loop is restored.

### Startup/main-menu status

Phase 8.27's corrected LZMA fast path is retained unchanged. Field logs confirm it is genuinely active, so the remaining black initialization interval is no longer explained by the LZMA wrapper alone. Phase 8.28 intentionally does not bypass the required initializer or change the stable 4,000-instruction scheduling profile.

The all-effects batch optimization targets the reported regression when graphics settings are enabled. If startup/main-menu lag remains after this build, it should be profiled as a separate path without disturbing the now-smooth in-game path.
