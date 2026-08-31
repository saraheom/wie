# WIPI Player

The current TestFlight workflow targets **WIPI Player 0.1.53 (Phase 8.53)**. This stabilization build is based directly on Phase 8.50 and preserves the verified Inotia1 global monster-reward overflow repair, save/revival compatibility, the established offline cash-shop protocol, and the existing Inotia2 compatibility/performance work.

Phase 8.53 keeps the stabilized Phase 8.52 Inotia1 reward/cash behavior unchanged and begins the next LGT compatibility pass. It implements generic LGT imported-interface method linking (needed by OZ `org/kwis/msf/io/Socket`) and adds explicit LGT native-startup/error markers. The 11-record Inotia1 cash catalog is unchanged: `무기강화 주문서`, `방어구강화 주문서`, `힘의 조각`, and `마법의 가지` remain quantity **10** per purchase.

See the phase-specific notes for compatibility history and TestFlight setup.
