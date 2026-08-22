# WIPI Player Phase 8.9.1 — Full Repository

This ZIP is intended to replace the repository working tree as a complete baseline.
It was reconstructed from the last full-repository snapshot and the sequential
Phase 7.18 through Phase 8.9 update artifacts, then corrected for the Phase 8.9
Rust lifetime build failure.

## Why this full repo exists

The failed TestFlight run checked out commit `c2cbca8d5faf6d12828bf88acdfa95f8399b6fa0`
and failed during `npm run build:prod`, before Apple signing or upload. The compiler
reported a lifetime error in:

    wie_ktf/src/runtime/wipi_c/method_table.rs

The source used an async closure holding `&mut dyn WIPICContext`. Phase 8.9.1 uses
an async function item instead, matching the higher-ranked WIPI method adapter.

The reconstructed repository also restores the clean-WASM TestFlight step that
was present in the known-good full-repo baseline, but updates its source checks
from the old Phase 7.17 markers to the current Phase 8.9/8.9.1 markers.

## Preserved Phase 8.9 behavior

- KTF Inotia 2 PID `PD007974`: `MC_knlGetAccessLevel` returns `1`.
- First access-level call logs `[PHASE8_9_ACCESS]`.
- Inotia 2 `i_pack.dat` mode-4 rebuild truncates stale persistent content first.
- Phase 8.8 KTF stream seek behavior remains.
- Earlier Inotia 1 save compatibility and MapleStory behavior remain in the tree.

## Recommended GitHub use

1. Back up or rename the current local checkout.
2. Extract this ZIP.
3. Use the extracted `wie-phase8.9.1-fullrepo` directory contents as the repository root.
4. Commit all changes and push to `main`.
5. Run the **iOS TestFlight** workflow.
6. In the workflow log, confirm the source-verification step finds both
   `PHASE8_9_IPACK_CREATE` and `PHASE8_9_ACCESS`.

Do not overlay this full repo onto an older checkout file-by-file; replace the
working tree from this baseline (while preserving your `.git` directory if you
are updating an existing clone).
