# WIPI Player Phase 8.20 — Dual Inotia follow-up

Phase 8.20 is based on the complete Phase 8.19 repository.

## Inotia 2 — shared packaged-resource cache

Phase 8.19 introduced an Inotia-2-only cache for packaged resources, but the
KTF SVC dispatcher constructed a fresh `KtfWIPICContext` for every WIPI C SVC.
That also created a fresh empty cache every time, so field logs showed repeated
`LOAD` events and no reuse.

Phase 8.20 stores one `Arc<Mutex<...>>` cache in the registered WIPIC SVC
handler state and clones that same Arc into all short-lived contexts and
callbacks. The cache covers the large install/runtime resources plus the small
immutable appinfo/envinfo/cert fallbacks. The 4,000-instruction Inotia 2 run
slice and the known-good Phase 8.18/8.19 install behavior are unchanged.

Expected marker:

`[PHASE8_20_INOTIA2_RESOURCE_CACHE_SHARED]`

After a resource's first `[PHASE8_19_INOTIA2_RESOURCE_CACHE] LOAD`, later
accesses in the same launch should no longer produce another LOAD.

## Inotia 1 — command-1 legacy response validation

Phase 8.19 confirmed that the game fully reads the 27-byte local command-1
response and then enters the common network error cleanup. Static analysis of
PD005362 resolves the first post-parse gate at guest `0x00117418`: the old
carrier/server validator returns zero in the offline environment, which sends
the client to error 2009 before the normal command-1 continuation.

For only AID `010100D3`, PID `PD005362`, and the known 431,008-byte native
image, Phase 8.20 changes that conditional branch to an unconditional branch
to the original normal continuation. The validator itself still executes so
its side effects are preserved. This does not fabricate a purchase result;
later outbound cash-shop commands remain packet-capture-only until their
format is observed.

Expected marker:

`[PHASE8_20_INOTIA1_CASH_CMD1_VALIDATION_BYPASS]`

A new one-shot cleanup diagnostic also dereferences the native cash protocol
status/error global:

`[PHASE8_20_INOTIA1_CASH_STATE]`

This gives the exact error code if the client reaches another rejection later.
