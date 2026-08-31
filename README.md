# WIPI Player

The current TestFlight workflow targets **WIPI Player 0.1.55 (Phase 8.55)**. This stabilization build preserves the verified Inotia1 global monster-reward overflow repair, save/revival compatibility, the established offline cash-shop protocol, and the existing Inotia2 compatibility/performance work.

Phase 8.55 keeps the generic LGT interface-link repair and stateful InputMethodHandler compatibility, then completes the OZ-imported `TextComponent` surface: `getMaxLength`, `setMaxLength`, `getString`, and `setString` now use real object state. `TextFieldComponent` and `TextBoxComponent` also retain their constructor text/constraint. Explicit LGT startup/error diagnostics remain enabled. The 11-record Inotia1 cash catalog is unchanged: `무기강화 주문서`, `방어구강화 주문서`, `힘의 조각`, and `마법의 가지` remain quantity **10** per purchase.

See the phase-specific notes for compatibility history and TestFlight setup.
