# WIPI Player Phase 8.13.1

Full-repository build correction for Phase 8.13.

## Fix

`wie_ktf/src/runtime/init.rs` now imports `wie_util::ByteRead` and
`wie_util::ByteWrite`, which are the traits that provide `ArmCore::read_bytes`
and `ArmCore::write_bytes` used by the exact Inotia 2 certificate-validator
patch.

No Phase 8.13 runtime behavior was otherwise changed:

- Inotia 2 `010100D5 / PD007974`: exact legacy validator bypass remains.
- Inotia 1 `010100D3 / PD005362`: KTF network slot 30 bridge remains.
- Existing Phase 8.12 cash-shop packet diagnostics remain enabled.
