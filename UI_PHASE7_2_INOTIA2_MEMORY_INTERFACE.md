# Phase 7.2 — Inotia 2 KTF incremental-memory compatibility

Inotia 2 successfully reaches the KTF emulator but its embedded C/C++ runtime requests the carrier extension `WIPICX_incMemInterface`. WIE previously returned null, causing the runtime to disable static allocation APIs (`new`, `malloc`, `free`) and the game to present a memory error.

This test build adds a one-entry incremental-memory interface backed by the existing ArmCore allocator. It also emits info-level diagnostics for the interface table, each incremental-memory request, returned guest address, and the WIPI reported total/free-memory APIs.

The implementation is intentionally instrumented because this KTF extension is outside the public WIPI C API. If Inotia 2 still fails, the exported app log will reveal the actual call ABI/arguments so the interface can be refined without patching the game binary.
