# UI / Runtime Phase 8.18

## Test plan

### Inotia 2
1. Launch the existing imported game without deleting saves.
2. Confirm the Phase 8.17 `메모리에러` regression is gone.
3. Observe the startup/install progress. The required native initializer is intentionally restored, but static database write amplification is removed, so this pass should be substantially faster.
4. Load the existing save and compare movement, attack, dialogue, system-menu opening, and map transitions.
5. Confirm shadow/weather/critical effects remain off.
6. Quit fully and launch again.

### Inotia 1
1. Open `시스템 → 캐쉬템 구매`.
2. Leave the resulting screen open for several seconds.
3. If a shop/list appears, navigate and select an item, but do not assume purchase completion until the log confirms the next protocol command.
4. Export diagnostics.

## Key markers

- `PHASE8_18_INOTIA2_INSTALL_WRITEBACK`
- `PHASE8_18_INOTIA2_EXEC_QUANTUM`
- `PHASE8_18_INOTIA1_CASH_INIT_RX`
- `PHASE8_18_INOTIA1_CASH_PROTOCOL`
