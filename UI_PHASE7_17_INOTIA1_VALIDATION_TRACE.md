# Phase 7.17 — Inotia 1 validation trace

This phase deliberately stops modifying Inotia 1 save bytes.

What changed:
- Removes Phase 7.16's title-specific full-record-replacement mutation.
- Keeps normal KTF CREATE/truncate semantics.
- Keeps synchronous write-through persistence.
- Adds a build marker: `[PHASE7_17]`.
- Adds dependency-free FNV-1a 64-bit fingerprints plus the first/last 16 bytes
  for every Inotia 1 save READ and WRITE.

Why:
The three exported save snapshots are all 328 bytes, including the known-good
pre-Terry save. WIE does not compute an 8-byte checksum/footer; it stores the
guest-provided bytes. Therefore checksum synthesis in the emulator would be
speculative and risks corrupting the save.

Test:
1. Clean Slot 1.
2. Save before Terry; verify Continue.
3. Load, talk to Terry, save.
4. Return to Continue.
5. Export the global log.

Compare WRITE and subsequent READ `fnv64` values for save0.dat. If they match
but Continue is empty, the storage layer is exonerated and the next target is
the guest-side ARM validation branch after `stream_read`.
