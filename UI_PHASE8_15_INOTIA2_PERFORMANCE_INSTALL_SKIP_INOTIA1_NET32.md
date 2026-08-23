# WIPI Player Phase 8.15 — Inotia 2 performance/install fast path + Inotia 1 KTF net32

Phase 8.15 continues from the complete Phase 8.14 repository.

## Inotia 2 (010100D5 / PD007974)

Phase 8.14 repaired the generated resource databases and the title now reaches normal gameplay. The latest gameplay log exposed a separate emulator-side performance problem: temporary native-loop/database diagnostics were still enabled at production verbosity. A ~30-second log contained thousands of `[NATIVE_LOOP]` WARN lines plus hundreds of per-read/per-seek database messages. On iOS these messages cross the WASM/WebView logging bridge and materially increase stutter during CPU/resource-heavy scenes.

Phase 8.15:

- reduces `[NATIVE_LOOP]` diagnostics to only the deep-hang threshold (16,384 exhausted 1,000-instruction chunks);
- demotes the old Inotia 2 per-open/read/write/seek and Phase 8.1–8.8 investigation traces to DEBUG while preserving correction/error markers;
- keeps Phase 8.14 cache repair, Phase 8.9 i_pack CREATE semantics, certificate/access-level/GetExecNames compatibility, and player save behavior unchanged.

### Repeated installation screen

The dump contains a complete preinstalled database image under `p/`. Existing users can still carry an older persistent `appinfo.dat` produced during the earlier compatibility phases. The bundled installed metadata is a 5-byte `appinfo.dat` whose final byte is `1`; a stale persistent copy leaves the title on its legacy installer path even though the expanded caches are already present.

For the exact Inotia 2 AID/PID only, Phase 8.15 repairs stale `appinfo.dat` from the bundled installed snapshot on a normal read. This does not touch player save databases.

Marker:

    [PHASE8_15_INOTIA2_INSTALL_SKIP]

After the metadata is repaired, subsequent launches should proceed directly into the normal title/start-menu path instead of rebuilding/showing the installation pass.

## Inotia 1 cash shop (010100D3 / PD005362)

Phase 8.14 successfully progressed through:

1. offline `MC_netConnect`;
2. `MC_netSocket`;
3. `MC_utilInetAddrInt` for the original dotted endpoint;
4. KTF legacy network slot 30;
5. the asynchronous slot-30 success callback.

The new crash occurs immediately after that callback. Static analysis of `client.bin138532` shows the callback continuation dereferencing network-interface offset `0x80`, i.e. KTF extension slot 32, and calling it as `(fd, buffer, length)`. The table previously ended at slot 30.

Phase 8.15 extends the KTF network table through slot 32. Slot 32 is title-scoped to Inotia 1 and routes to the existing offline socket-write packet capture. No historical server connection is made.

Markers:

    [PHASE8_15_INOTIA1_NET32]
    [PHASE8_12_CASH_TX]

The second marker should expose the original cash-shop request packet needed to implement a local response in the next phase.

## Scope

All compatibility relaxations remain limited to the known title paths. No intentional Heroes Lore 2 or MapleStory behavior changes are included.
