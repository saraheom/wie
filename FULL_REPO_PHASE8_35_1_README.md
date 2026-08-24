# WIPI Player Phase 8.35.1

This is the complete Phase 8.35 repository with a TestFlight CI verification correction.

The failed Phase 8.35 GitHub Actions run reached successful IPA generation. The failure was
the post-build sentinel check, not Rust compilation or Xcode export. Phase 8.35.1 keeps all
Phase 8.35 runtime changes unchanged and replaces the brittle binary string search with a
deterministic frontend marker that is checked in webpack output, generated iOS assets, and
the final IPA.

App marketing version remains 0.1.35 because the failed Phase 8.35 IPA was never uploaded.
The GitHub run/build number still increments normally.
