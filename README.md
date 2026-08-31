# WIPI Player

The current TestFlight workflow targets **WIPI Player 0.1.54 (Phase 8.54)**. This stabilization build is based directly on Phase 8.50 and preserves the verified Inotia1 global monster-reward overflow repair, save/revival compatibility, the established offline cash-shop protocol, and the existing Inotia2 compatibility/performance work.

Phase 8.54 keeps the Phase 8.53 generic LGT interface-link repair and adds the next OZ startup dependency: stateful `org/kwis/msp/lcdui/InputMethodHandler.getCurrentMode()I` support paired with the existing `setCurrentMode(int)` API. Explicit LGT startup/error diagnostics remain enabled. The 11-record Inotia1 cash catalog is unchanged: `무기강화 주문서`, `방어구강화 주문서`, `힘의 조각`, and `마법의 가지` remain quantity **10** per purchase.

See the phase-specific notes for compatibility history and TestFlight setup.
