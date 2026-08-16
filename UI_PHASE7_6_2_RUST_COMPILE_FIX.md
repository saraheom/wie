# Phase 7.6.2 — Rust Compile Fix

This is the same Phase 7.6 LGT compatibility + iOS safe-area build with two
Rust compile issues corrected in the new `memcmp` implementation.

## Fixed

1. `ArmCore::read_bytes()` is provided by the `wie_util::ByteRead` trait.
   Phase 7.6.2 imports that trait explicitly.

2. WIE's ARM `ResultWriter` supports `u32` guest return values, not Rust `i32`.
   `memcmp_lgt()` now returns `Result<u32>` and preserves C signed-result
   semantics by casting the signed difference to its 32-bit two's-complement
   representation before writing R0.

No LGT behavior, sprintf logic, WIPIC 0x19C handling, persistence, localization,
or safe-area behavior was otherwise changed.
