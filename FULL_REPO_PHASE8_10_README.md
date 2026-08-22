# WIPI Player full repository — Phase 8.10

This tree is a complete repository snapshot based on Phase 8.9.1 plus the
Inotia 2 KTF legacy authentication access-mask correction.

Key compatibility state retained:

- Inotia 1 save compatibility work through Phase 8.1.2.
- Inotia 2 KTF packaged-resource/database fixes through Phase 8.8.
- Phase 8.9 `i_pack.dat` CREATE/rebuild truncation fix.
- Phase 8.9.1 lifetime-safe `MC_knlGetAccessLevel` implementation.
- Phase 8.10: KTF Inotia 2 (`010100D5` / `PD007974`) returns `0xBC` from
  `MC_knlGetAccessLevel`, matching the exact mask required by the game's
  native authentication code and avoiding its error-1001 branch.

The iOS TestFlight workflow forces a clean WASM rebuild and verifies the
Phase 8.9 database marker and Phase 8.10 access-mask marker before packaging.
