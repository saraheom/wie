# Phase 8.86.1 — Rust compile fix

This is a CI-only correction to Phase 8.86.

GitHub Actions failed while compiling `wie_lgt/src/runtime/java.rs` because the new Phase 8.86 Java String decoder used `read_generic(core, array_fields)` immediately before `.min(4096)`, leaving Rust unable to infer the generic read type.

The decoder now supplies explicit generic types (`u32`, `u16`, `RawJavaClassInstance`, `RawJavaClass`, and `RawJavaClassDescriptor`) for all newly added `read_generic` calls. Runtime behavior and Phase 8.86 diagnostics/fixes are otherwise unchanged.

TestFlight marketing version remains `0.1.86` because the previous build never completed or uploaded.
