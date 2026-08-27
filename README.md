# WIPI Player

The current TestFlight workflow targets **WIPI Player 0.1.50 (Phase 8.50)**. This stabilization build is based directly on Phase 8.49 and preserves the verified Inotia1 global monster-reward overflow repair, save/revival compatibility, the established offline cash-shop protocol, and the existing Inotia2 compatibility/performance work.

Phase 8.50 cleans up the temporary EXP/spawn diagnostic UI and reduces reward logging to actual overflow repairs only. The normal Inotia1 cash catalog is now 9 records: the proven utility items plus `힘의 조각` and `마법의 가지`; `초보용 용사의 인장` has been removed. Each purchase of either material requests **10 units** through the game's authentic command-31 quantity byte.

A synthetic `100000 골드` entry is intentionally **not** included in this stabilization build. Gold is not represented as an ordinary cash-catalog inventory item, and a safe game-native currency update path has not yet been verified.

The iOS workflow builds the WIE WebAssembly core, packages the Tauri iOS application, signs it with the configured Apple credentials, verifies the final IPA metadata, and uploads it to TestFlight.
