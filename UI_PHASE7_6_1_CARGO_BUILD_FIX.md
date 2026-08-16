# Phase 7.6.1 — Cargo Manifest Build Fix

This is the same Phase 7.6 LGT compatibility + iOS safe-area build, with only the
Rust dependency declaration corrected.

## Failure fixed

Phase 7.6 used:

    encoding_rs = { workspace = true }

in `wie_lgt/Cargo.toml`, but the root workspace does not define
`workspace.dependencies.encoding_rs`.

Phase 7.6.1 now matches the existing `wie_wipi_c` dependency declaration:

    encoding_rs = { version = "^0.8", default-features = false, features = ["alloc"] }

`Cargo.lock` is also updated so the `wie_lgt` package explicitly lists
`encoding_rs`.

No emulator behavior, LGT ABI implementation, save handling, UI behavior, or
safe-area logic was otherwise changed.
