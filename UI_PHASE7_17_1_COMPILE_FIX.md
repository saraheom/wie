# Phase 7.17.1 — compile fix

GitHub Actions failed in `wie_wipi_c/src/api/database.rs` because the Phase 7.17
READ diagnostic referenced a nonexistent local variable named `handle_bytes`.

`stream_read` already has the exact returned bytes in its local `data` buffer:

```rust
let mut data = vec![0u8; take as usize];
context.read_bytes(handle.buffer_ptr + handle.read_cursor, &mut data)?;
context.write_bytes(buf_ptr, &data)?;
```

The diagnostic now fingerprints `&data[..]` directly.

No save semantics or emulator behavior are otherwise changed from Phase 7.17.
