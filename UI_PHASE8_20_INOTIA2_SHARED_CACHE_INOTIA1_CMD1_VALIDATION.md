# UI / Runtime Phase 8.20

- Inotia 2: make the Phase 8.19 packaged-resource cache truly shared across
  KTF WIPIC SVC contexts for one launch; include appinfo/envinfo/cert fallbacks.
- Inotia 1: bypass only the obsolete command-1 post-parse carrier validation
  branch, preserving the validator call and original success continuation.
- Inotia 1: log the native protocol error/status global on socket cleanup.
- Preserve the working 4,000-instruction Inotia 2 execution quantum.
- Preserve Phase 8.18/8.19 installer/resource writeback behavior; no whole
  installer bypass is reintroduced.
