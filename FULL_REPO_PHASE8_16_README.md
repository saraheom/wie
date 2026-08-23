# WIPI Player Phase 8.16 Full Repository

Phase 8.16 continues the exact-title compatibility work for the KTF releases of Inotia 1 and Inotia 2.

## Inotia 2 (010100D5 / PD007974)

### Stop the repeated installation/rebuild pass

Phase 8.15 changed `appinfo.dat`, but the title still entered its cache verification/rebuild state machine. Phase 8.16 patches the exact native decision at guest address `0x00144e6a` only after the normal `i_pack.dat` loader has run. The loader is preserved, so its process-local resource globals are still initialized; only the redundant verification branch is skipped.

Expected marker:

```
[PHASE8_16_INOTIA2_INSTALL_VERIFY_BYPASS]
```

The patch is guarded by AID, PID, native-image size, and expected original instruction bytes.

### Performance profile

The three expensive in-game effects that materially improved performance during device testing are forced off for this title:

- Shadow effect
- Weather effect
- Critical effect

Only the low three bits of byte 3 in `envinfo.dat` are cleared; other preferences are retained.

Expected marker:

```
[PHASE8_16_INOTIA2_PERF_PROFILE]
```

Phase 8.16 also raises this title's native ARM execution slice from 1000 to 4000 guest instructions before yielding. This reduces async scheduler/WebView overhead without changing the timing policy for other games.

Expected marker:

```
[PHASE8_16_INOTIA2_EXEC_QUANTUM]
```

## Inotia 1 (010100D3 / PD005362)

### Correct KTF cash-shop socket ABI

The Phase 8.15 trace established that the two carrier-extension slots were reversed:

- slot 31 (`+0x7c`) = SEND
- slot 32 (`+0x80`) = RECEIVE

Phase 8.16 corrects that mapping. The original client is server-first, so the local offline bridge supplies the smallest valid command-0 greeting (`00 03 00`). This should allow the original client to generate its next request instead of immediately reporting a connection failure.

Expected markers:

```
[PHASE8_16_INOTIA1_NET32_RX]
[PHASE8_16_INOTIA1_NET31_TX]
[PHASE8_12_CASH_TX]
```

No external network connection is made.

This phase intentionally does **not** invent catalog or purchase-response packets. The outbound request captured after the local greeting is the evidence needed to implement a faithful offline/free cash-shop response in the following phase.

## Test plan

1. Build and install through the existing iOS TestFlight workflow.
2. Inotia 2: launch twice. Confirm the installation progress pass no longer appears on the second launch and compare movement/dialogue/menu/map-transition responsiveness.
3. Inotia 2: open graphics settings and confirm shadow/weather/critical remain off.
4. Inotia 1: open System -> Cash Item Purchase and leave it active for several seconds.
5. Export diagnostics. The most important Inotia 1 marker is `[PHASE8_12_CASH_TX]` after `[PHASE8_16_INOTIA1_NET32_RX]`.
