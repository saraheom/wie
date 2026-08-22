# WIPI Player Phase 8.14 — Dual Inotia compatibility

Phase 8.14 continues from the complete Phase 8.13.1 repository.

## Inotia 2 (010100D5 / PD007974)

The Phase 8.13 certificate bypass successfully reaches the title/menu and gameplay setup screens. The next failure was traced to generated KTF database caches being appended repeatedly instead of being truncated on `MC_DB_CREATE` (mode 4).

For example, `filetext.dat` has a compact 93,067-byte source in the JAR and a canonical 301,682-byte expanded database under `p/`. Older WIE behavior preserved the current record on CREATE whenever a packaged resource existed, so repeated launches produced `93,067 + N * 301,682` bytes. The test log reached 2,808,205 bytes, exactly nine appended expanded copies after the compact seed. Similar growth affected `eventdata.dat`, `i_mapfeature.dat`, and `i_tile.dat`.

Phase 8.14:

- applies true create/truncate semantics to the four Inotia 2 generated caches;
- keeps the existing Phase 8.9 `i_pack.dat` truncate fix;
- restores an already-polluted cache from the canonical expanded `p/<name>` snapshot when a non-CREATE open sees the wrong length;
- seeds a missing generated cache from the canonical `p/` snapshot instead of the compact JAR source;
- logs `[PHASE8_14_INOTIA2_CACHE_CREATE]` and `[PHASE8_14_INOTIA2_CACHE_RESTORE]`.

This is designed to repair existing installs without erasing unrelated save databases.

## Inotia 1 cash shop (010100D3 / PD005362)

Static analysis of the cash-shop callback shows that after `MC_netSocket` the title calls the utility interface at offset `0x10`, i.e. slot 4 `MC_utilInetAddrInt`, before it reaches the Phase 8.13 KTF network slot 30. That utility slot was still a fatal stub, explaining why the Phase 8.13 NET30 marker never appeared.

Phase 8.14 implements `MC_utilInetAddrInt`:

- dotted IPv4 strings are converted to a 32-bit address;
- for this exact obsolete Inotia 1 cash-shop path, a non-dotted or unreadable historical endpoint maps to an offline loopback placeholder instead of attempting DNS or contacting an external server;
- the existing Phase 8.13 network slot 30 and Phase 8.12 packet-capture bridge remain unchanged;
- logs `[PHASE8_14_INOTIA1_INETADDR]` / `[PHASE8_14_INETADDR]`.

The expected next Inotia 1 diagnostic progression is:

1. `[PHASE8_12_INOTIA1_NET]`
2. `[INOTIA1_CASH_NET] MC_netSocket ...`
3. `[PHASE8_14_INOTIA1_INETADDR]` or `[PHASE8_14_INETADDR]`
4. `[PHASE8_13_INOTIA1_NET30]`
5. ideally `[PHASE8_12_CASH_TX]`, exposing the original client packet for the next offline-shop protocol phase.

## Scope

All Inotia-specific relaxations remain guarded to their known AID/PID paths. No Heroes Lore 2 or MapleStory compatibility behavior is intentionally changed.
