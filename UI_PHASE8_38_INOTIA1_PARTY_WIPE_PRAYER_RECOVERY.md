# Phase 8.38 test plan — Inotia1 party wipe

1. Verify TestFlight version `0.1.38` and runtime marker `PHASE8_38_RUNTIME_SENTINEL`.
2. Confirm ordinary character movement remains as smooth as Phase 8.37.
3. Open the normal cash shop once and confirm the same 12-item Phase 8.37 catalog is unchanged.
4. Reproduce a total-party wipe without `부활의 기도문`.
5. Select the prayer revival option. The emergency cash session should show `부활의 기도문` rather than the normal catalog.
6. Cancel/back once. It should return to the original death prompt instead of freezing or crashing.
7. Re-enter the prayer purchase path, purchase `부활의 기도문`, return to the death prompt, and select prayer revival again. The original state-13 revival path should now be able to find and consume the item.
8. Save/exit/relaunch afterward and verify Continue remains valid.
9. Separately import the already-broken 8/25 backup and try Continue. If it still crashes, export the Phase 8.38 log; do not overwrite the known-good backup.

Relevant markers:
- `PHASE8_38_INOTIA1_WIPE_CASH_STATE`
- `PHASE8_38_INOTIA1_CASH_CATALOG`
- `PHASE8_38_INOTIA1_CASH_PURCHASE`
- `PHASE8_38_INOTIA1_WIPE_CASH_CANCEL_RECOVERY`
- `PHASE8_38_INOTIA1_CASH_EXIT_CLEANUP`
