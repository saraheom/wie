# Phase 8.44 note

The current TestFlight workflow targets WIPI Player 0.1.44. Phase 8.44 widens the read-only Inotia1 EXP diagnostic after the Phase 8.43 field log showed no candidate EXP writes. It now observes both 16-bit and 32-bit native stores and removes the old >=4096 signal floor while preserving the underlying Phase 8.42 gameplay behavior. See `FULL_REPO_PHASE8_44_README.md` for markers and the field-test procedure.
