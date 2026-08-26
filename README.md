> Current TestFlight development phase: **8.41** — Inotia1 exact Continue-position rescue for the preserved 2026-08-26 progress backup; Phase 8.40 prayer/revival fixes and the Phase 8.37 gameplay/catalog baseline are preserved. See `FULL_REPO_PHASE8_41_README.md`.

> **Phase 8.38 (Inotia1 party-wipe prayer recovery):** preserves the validated Phase 8.37 performance/name/12-item normal-cash baseline. When the original death UI enters native state 14 because `부활의 기도문` (item 0x219) is missing, the offline bridge exposes a one-item emergency prayer catalog. Backing out mirrors the title's own state-14 CLEAR transition back to the death prompt before command-123 cleanup, instead of leaving the outer death state stranded.

> **Phase 8.37 (Inotia 1 performance rollback + complete safe cash catalog):** preserves the now-validated `자원 교환권 -> 이노티아` main-name repair, removes the Phase 8.36 per-keydown PC/LR party-wipe probe so normal movement returns to the pre-diagnostic input path, and uses all 12 validated cash-catalog slots for the essential items including the four missing equipment/cosmetic entries. The cash entry header uses the previously working Phase 8.33 `[0,1]` compatibility state; every command-30 request maps to the same 12-record physical catalog, so no item requires paging. See `FULL_REPO_PHASE8_37_README.md`.

> **Phase 8.36 (Inotia 1 cash-entry + performance cleanup + party-wipe probe):** keeps the safe one-page nine-item catalog but restores the command-30 page fields to the field-proven ordering (`current_page=0`, `max_page_index=0`). It removes the broad Phase 8.35 name scans/hot-path tracing and keeps only an exact two-caller main-name repair for `자원 교환권 -> 이노티아`. It also adds a diagnostic-only key-down PC/LR marker for reproducing the separate total-party-death stall. See `FULL_REPO_PHASE8_36_README.md`.

# WIE

## Phase 8.36 Inotia 1 compatibility notes

Phase 8.36 fixes the Phase 8.35 cash catalog header regression and removes the expensive name-recovery instrumentation that caused whole-game lag. The nine-item single-page layout remains below the 12-record client capacity. The main-character name fix is retained with only exact callsite/string matches. A lightweight input-state probe is included for the independent all-party-dead stall.

> **Phase 8.35.3 (Phase 8.35 runtime + TestFlight CFBundleVersion verification correction):** keeps the Phase 8.35 runtime unchanged. The previous run successfully exported the IPA, whose `CFBundleVersion` was `0.1.35.<build-number>`; the workflow had incorrectly expected only the bare numeric build number. Phase 8.35.3 verifies the Tauri-composed iOS build version correctly.

## Phase 8.35.3 TestFlight workflow correction

Phase 8.35.3 changes only the post-build IPA identity assertion. Tauri 2 currently emits `CFBundleVersion` as `<marketing-version>.<build-number>` (for example `0.1.35.85`). The workflow now validates that composed value while keeping the bundle identifier and marketing version checks strict. See `FULL_REPO_PHASE8_35_3_README.md`.

## Phase 8.35.2 TestFlight workflow correction

Phase 8.35.2 keeps the Phase 8.35 runtime unchanged and removes the invalid assumption that Tauri creates `wie_app/gen/apple/assets`. The previous 8.35.1 run had already exported a signed IPA before that check failed. The corrected workflow validates the Phase 8.35 runtime in the clean raw/Webpack WASM, rebuilds a fresh Apple project, and validates the final IPA package identity directly. See `FULL_REPO_PHASE8_35_2_README.md`.


## Phase 8.33.1 TestFlight workflow correction

Phase 8.33.1 keeps Phase 8.33 runtime behavior unchanged and fixes stale Phase 8.32 three-page catalog assertions in the iOS TestFlight workflow. See `FULL_REPO_PHASE8_33_1_README.md`.


## Phase 8.33 Inotia 1 two-page compatibility / heap-name recovery notes

Phase 8.33 responds to the Phase 8.32 field trace rather than extending the
three-page guess. The trace shows that the guest repeatedly emits command-30
page 1 while stuck at 3/1 and only reaches page 2 after an item-detail/back
transition. The offline catalog is therefore collapsed to two safe 9-record
pages (still below the proven 12-record overwrite boundary), and every nonzero
page request maps to page 1. The phase also probes WIE's live 16/32-byte
small-object heap at the first authentic command-5 transfer, before catalog
strings are copied, and repairs the two corrupted character names only when
the exact corrupt pair is unique. See `FULL_REPO_PHASE8_33_README.md`.

## Phase 8.32 Inotia 1 page-bound / character-name repair notes

Phase 8.32 preserves the validated Phase 8.30 special-item behavior and the
Phase 8.31 six-record catalog safety limit, corrects the command-30 page-bound
field so the original UI sees native page indices 0/1/2 instead of a phantom
fourth page, and adds an exact-match in-memory recovery for the two persisted
character names corrupted by Phase 8.28. See `FULL_REPO_PHASE8_32_README.md`.

## Phase 8.31 Inotia 1 cash paging / name-recovery notes

Phase 8.31 preserves the validated Phase 8.30 network-item fixes, changes the
18-item Inotia 1 cash catalog to the title's observed three six-item pages
(0/1/2), and adds a non-destructive rendered-name probe for comparing the
August 22 clean backup with the August 24 corrupted backup. See
`FULL_REPO_PHASE8_31_README.md`.


## Phase 8.24 Inotia compatibility/performance notes

Phase 8.24 completes Inotia 1's empty command-2 transfer with the original command-4 finalization path and handles the observed command-123/command-30 cash-shop re-entry sequence. For Inotia 2, it removes the high-volume Phase 8.23 profiler from normal gameplay and applies a behavior-preserving ARM interpreter dispatch optimization while retaining the safe 4,000-instruction profile and hidden install progress UI. See `FULL_REPO_PHASE8_24_README.md`.

## Phase 8.13.1 local compatibility notes

This snapshot includes the Phase 8.13 dual Inotia compatibility work plus the
Phase 8.13.1 Rust trait-import compile correction. It bypasses the exact Inotia 2 KTF legacy carrier-certificate
validator and adds the missing Inotia 1 KTF network slot 30 used by the
cash-shop connection path. See `UI_PHASE8_13_DUAL_INOTIA_COMPAT.md`.
[Homepage](https://wie-site.dlunch.net) | [Try in browser](https://wie.dlunch.net)

A standalone web-based emulator for old mobile apps based on WIPI, SKVM or J2ME.

This project is dedicated to digital preservation and educational research. Our goal is to revive the legacy of classic mobile games and allow them to be experienced in modern web environments.

- [Contribution guide](https://github.com/dlunch/wie/blob/main/CONTRIBUTING.md)
- Architecture docs: [Emulator](docs/architecture.md) | [KTF](docs/ktf.md) | [LGT](docs/lgt.md)

## Frontend

The web and Android/iOS frontends are maintained in this repository under `wie_web` and `wie_app`.

```bash
npm install
npm run build:dev   # development web build
npm run build:prod  # production web build
npm start           # web development server
```

## Related projects

- [RustJava](https://github.com/dlunch/RustJava)
- [smaf](https://github.com/dlunch/smaf)
