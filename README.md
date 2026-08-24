# WIE

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
