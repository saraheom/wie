# Phase 8.49 note

The current TestFlight workflow targets WIPI Player 0.1.49. Phase 8.49 is based directly on Phase 8.48 and preserves the confirmed Inotia1 cash-shop catalog (including `힘의 조각` and `마법의 가지`), save/revival compatibility, Inotia2 behavior, and the optional exact EXP/entity diagnostics.

Phase 8.49 adds the first actual Inotia1 EXP repair. The original monster base-reward helper at guest `0x001281ec` performs its intermediate multiplications in signed 32-bit arithmetic. Higher monster parameters can overflow the numerator before the final signed division. The repair replaces that helper for the verified monster-constructor caller (`LR=0x00126245`) with the same formula evaluated using wide intermediates. It is global to the constructor path: it is not keyed to `수호물 K34`, a monster name, or a specific entity slot.

The hook always reconstructs the original wrapped result and the wide result. If they agree, the original value is returned unchanged. If they diverge, the wide result is returned and `PHASE8_49_INOTIA1_REWARD_OVERFLOW_REPAIR` is logged. The optional Arm/Reset EXP + Spawn Trace button remains available for verification, but the repair itself is automatic and does not require arming.

See `FULL_REPO_PHASE8_49_README.md` for test instructions and the reconstructed formula.
