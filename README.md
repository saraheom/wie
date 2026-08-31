# WIPI Player

The current TestFlight workflow targets **WIPI Player 0.1.61 (Phase 8.61)**. It preserves the Phase 8.60 LGT class-initialization re-entrancy repair and all existing Inotia1/Inotia2 compatibility work.

Phase 8.61 targets the next OZ/천공의 기사단 startup blocker exposed by the 8.60 diagnostic log. The LGT Java import ABI is aligned with current upstream semantics for import `0x12` (class assignability) and import `0x21` (throw exception), and the `0x12` path now logs the actual guest pointers plus bounded pointer-word diagnostics when the class-name argument is malformed. This keeps the next test evidence-driven instead of hiding the guest exception.

See the phase-specific notes for compatibility history and TestFlight setup.
