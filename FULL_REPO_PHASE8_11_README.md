# WIPI Player full repository — Phase 8.11

This is a complete repository snapshot based on Phase 8.10.

Phase 8.11 keeps all earlier compatibility work and adds a narrowly scoped KTF
Inotia 2 implementation for kernel slot 2, `MC_knlGetExecNames`.

The Phase 8.10 log proves the `0xBC` access-level bypass was accepted: the game
then advanced directly into `MC_knlGetExecNames`, where the previous generic
fatal stub terminated the runtime. Static analysis of the exact Inotia 2
caller shows it requests the current AID (`010100D5`) with null version/vendor
filters and a 300-byte output buffer. The caller expects a 21-byte executable
name whose two eight-character AID fields match.

This build returns the archive-consistent self executable name:

`010100D5/010100D5.jar`

as a double-NUL-terminated WIPI string list and returns one match.

The TestFlight workflow retains the forced clean WASM rebuild and now verifies
both Phase 8.10 and Phase 8.11 source markers before packaging.
