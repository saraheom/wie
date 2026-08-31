# WIPI Player

The current TestFlight workflow targets **WIPI Player 0.1.60 (Phase 8.60)**. This build preserves the verified Inotia1 global monster-reward overflow repair, save/revival compatibility, 11-record offline cash-shop catalog with the four bulk items at quantity 10, and the existing Inotia2 compatibility/performance work.

Phase 8.60 preserves the OZ/LGT fixes from Phases 8.53–8.59 and repairs recursive LGT class initialization. `InitializeClass` now marks a class as state 4 (initializing) before invoking its initializer callback, safely returns on same-class re-entry, promotes the class to state 5 only after successful completion, and restores the previous state if the callback fails.

See the phase-specific notes for compatibility history and TestFlight setup.
