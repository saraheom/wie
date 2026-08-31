# UI Phase 8.60 — OZ LGT Class-Initialization Re-entrancy Repair

No UI changes. This is a generic LGT VM runtime repair. During `InitializeClass`, classes are now marked state 4 (initializing) before `<clinit>` runs. Re-entrant initialization of the same class returns without recursively invoking the callback, and successful completion transitions to state 5.
