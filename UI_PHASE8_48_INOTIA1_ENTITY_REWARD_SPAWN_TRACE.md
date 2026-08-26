# Phase 8.48 UI note

The testing-only diagnostics button is renamed to **Arm/Reset EXP + Spawn Trace**. It still arms the exact player EXP watchpoint, but now also snapshots and watches the entity slot base-reward fields. The settings note instructs testers to arm before a map transition or monster-spawn event and export after the monsters appear. No production gameplay UI is changed.
