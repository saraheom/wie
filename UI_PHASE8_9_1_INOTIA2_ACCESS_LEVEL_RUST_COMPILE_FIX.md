# Phase 8.9.1 — Inotia 2 access-level Rust compile fix

## Why 8.9 failed in TestFlight

The Phase 8.9 functionality was correct in intent, but the implementation of
`MC_knlGetAccessLevel` used an async closure that captured
`&mut dyn WIPICContext`. The WIPI method adapter requires a higher-ranked
callable whose future is valid for each context borrow lifetime. Rust therefore
rejected the closure with:

    error: lifetime may not live long enough
    wie_ktf/src/runtime/wipi_c/method_table.rs

This happened during the WebAssembly/Rust build, before Apple project
generation, signing, archive, or TestFlight upload.

## Fix

The compatibility body is now an `async fn` function item:

    get_access_level_compat_impl

and `get_access_level_compat()` converts that function item with
`into_body()`. This satisfies the `for<'a> FnHelper<'a, ...>` requirement in
`wie_wipi_c::method` without changing the runtime behavior.

## Runtime behavior preserved

- PID `PD007974` receives access level `1`.
- `[PHASE8_9_ACCESS]` logs the first call and CPU caller snapshot.
- Other titles retain the previous `MC_knlGetAccessLevel` unimplemented path.
- Phase 8.9 `i_pack.dat` mode-4 truncation/rebuild behavior is unchanged.
- Phase 8.8 KTF seek semantics and all earlier compatibility work are retained.

## Repository packaging

This package is a full repository snapshot reconstructed from the last full-repo
baseline and all subsequent phase patches through 8.9, then corrected with this
compile fix. Future phase deliverables should use the same full-repo packaging
model rather than patch-only ZIPs.
