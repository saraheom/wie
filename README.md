# WIE


## Phase 8.22 Inotia compatibility/performance notes

Phase 8.22 corrects Inotia 1's offline cash-shop response framing (including the common state/result byte the native dispatcher expects), hides only Inotia 2's obsolete installation progress renderer while preserving required initialization, and optimizes the ARM/WASM framebuffer hot paths used by animation, skills, and map transitions. See `FULL_REPO_PHASE8_22_README.md`.

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
