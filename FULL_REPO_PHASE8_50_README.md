# Phase 8.50 — Stabilized Reward Repair + Bulk Resources

Phase 8.50 is a cleanup/stabilization release based on Phase 8.49.

## Retained fixes
- Global Inotia1 reward-overflow repair at the verified common reward-construction helper.
- Existing Inotia1 save/revival and offline cash-shop compatibility.
- Existing Inotia2 compatibility/performance work and other title fixes.

## Cleanup
- Removed the temporary `Arm/Reset EXP + Spawn Trace` control from the player diagnostics UI.
- The underlying diagnostic plumbing remains disarmed/dormant to minimize risky cross-crate deletion.
- Normal monster reward calculations no longer emit an INFO record; only real overflow repairs emit `PHASE8_50_INOTIA1_REWARD_OVERFLOW_REPAIR`.

## Cash catalog
The normal single-page catalog now has 9 records. `초보용 용사의 인장` was removed. `힘의 조각` and `마법의 가지` remain and use command-30/31 quantity field 10, so one purchase requests ten units through the native game purchase path. Prices remain zero. The emergency `부활의 기도문` catalog remains separate and unchanged.

## Gold
No synthetic `100000 골드` catalog record is included. Gold is not a normal inventory-item record in the verified shop data, so adding it safely requires a separate currency-path investigation rather than fabricating an item name or mutating save bytes blindly.

## Recommended smoke test
1. Buy 힘의 조각 once and verify inventory increases by 10.
2. Buy 마법의 가지 once and verify inventory increases by 10.
3. Confirm 초보용 용사의 인장 is absent.
4. Kill one previously overflow-affected monster and verify EXP increases.
5. Confirm save/continue and ordinary gameplay remain stable.
