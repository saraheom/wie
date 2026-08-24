# UI / Runtime Phase 8.32

## Inotia 1

- Command-30 catalog page header now advertises highest valid index `2` for three pages.
- Valid request indices remain `0`, `1`, and `2`; every response contains six records.
- Invalid page indices fall back to page 0 rather than clamping to page 2.
- Exact persisted name recovery: `자원 교환권` -> `이노티아`; `초보용 용사의 인장` -> `기사`, only in the two statically resolved adjacent character-name slots and only on complete EUC-KR byte matches.
- No new cash-shop exit response is synthesized; stale-description behavior should be retested after page-state correction.

## Inotia 2

No behavior change in this phase.
