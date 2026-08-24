# WIPI Player Phase 8.34 — Inotia1 cleanup and diagnostics

This is a complete repository, based on Phase 8.33.1.

## Changes

- **Cash-shop exit cleanup:** outbound command 123 now queues `00 04 7b 01`. Static recovery of the Inotia1 receive jump table shows command 123 routes to the native generic cleanup handler at guest `0x001171fa`; the previous no-response behavior was incorrect.
- **Resource exchange ticket:** `자원 교환권` is treated as a network/server-account balance operation, not a normal inventory grant. It is removed from the offline shop catalog, and any existing ticket use gets command-89 result/state 0 rather than a fake success.
- **Name recovery:** Phase 8.33's 16/32-byte-only heap probe is replaced by a bounded probe of mapped native runtime data/BSS, in-use WIE allocations, and one-level GOT-reachable structures. Only a unique pair in mapped native runtime state is auto-repaired. Exact RVCT `strcpy`/`strlen` hooks also log corrupt base names; complete `name(class)` matches are safe to repair in place.
- **Page navigation root fix:** native parser/UI recovery proves command-30 uses `page_count` at GOT `0x510` and zero-based `current_page` at GOT `0x4a4`. Earlier phases sent these two fields in the opposite order, exactly explaining the `3/0`-style display and blocked forward arrow. Phase 8.34 sends `[2,0]` on page 0 and `[2,1]` on page 1. Raw page globals remain logged for validation.
- Inotia2 code paths are unchanged.

## Catalog

Page 0: 9 records. Page 1: 8 records. `자원 교환권` is omitted because its effect depends on a historical server-side resource/account balance that has no single-player inventory representation.

## TestFlight compile gate

This environment does not contain a Rust toolchain. The GitHub TestFlight workflow remains the Rust/WASM compilation gate. The workflow sanity assertions were updated for Phase 8.34 before packaging.
