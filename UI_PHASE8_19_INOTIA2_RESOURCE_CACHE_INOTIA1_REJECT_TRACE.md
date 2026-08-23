# UI / Runtime Phase 8.19

## Inotia 2
- Retain the Phase 8.18 working native initializer and buffered install writeback.
- Preserve the exact `base + 8` runtime footer produced by the four generated caches instead of repeatedly stripping it.
- Treat both `base` and `base + 8` as valid installed lengths; all other abnormal lengths still use the Phase 8.14 repair path.
- Add a shared per-launch cache for the five hot canonical install resources.
- Return shadow/weather/critical settings to normal user control; do not rewrite `envinfo.dat`.
- Do not reintroduce the unsafe Phase 8.17 installer-call bypass.

## Inotia 1
- Retain the offline network bridge and Phase 8.18 command-1 response for reproducible parser behavior.
- Trace the exact native close/error call site before receive-state reset.
- Do not fabricate catalog or purchase packets until the command-1 rejection branch is identified.
