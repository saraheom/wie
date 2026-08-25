# Phase 8.40 test notes

- Baseline: preserve Phase 8.37 Inotia1 performance, 12-record normal cash catalog, main-name repair, and Phase7_21 save behavior.
- Party wipe: emergency state-14 origin remains latched only until a `부활의 기도문` command-31 success frame is fully delivered.
- On successful emergency purchase: clear the cancel latch and leave command-123 cleanup immediately pending for the guest's next read-callback registration. Do not rewrite native state 13/1.
- Pre-purchase CLEAR still mirrors the original state14 -> state11, selection0 cancellation.
- Resurrection-scroll crash: exact hook at guest 0x00131cb2 emulates `LDR R0,[SP,#0x4c]`, validates R8 context fields, repairs R4=R8, then continues to BL 0x0011dfb8.
- Exception-only `PHASE8_39_ARM_FAULT_CONTEXT` remains available if a different fault survives.
