# UI Phase 8.16 — Inotia 2 Performance/Install Skip + Inotia 1 Offline Cash Handshake

## Goals

- Preserve the now-working Inotia 2 gameplay path.
- Avoid rebuilding already-installed Inotia 2 resource caches every launch.
- Apply the user-tested low-cost Inotia 2 graphics profile automatically.
- Reduce emulator scheduling overhead for Inotia 2 without changing other titles.
- Correct the Inotia 1 KTF cash-shop SEND/RECEIVE slot mapping and advance the dead-server protocol locally.

## Inotia 2

The old Phase 8.15 `appinfo.dat` override is removed. It did persist the alternate metadata but did not control the native cache-install state machine.

The exact KTF native image now branches directly to the existing "resources valid" return path at `0x00144e6a`, after the normal `i_pack.dat` loader has executed. This is deliberately narrower than skipping startup code wholesale.

The graphics profile clears `envinfo.dat[3] & 0x07`, corresponding to the three settings observed in device testing: shadow, weather, and critical effects. The title-specific native execution quantum is also raised to 4000 instructions per async slice.

## Inotia 1

Runtime tracing from Phase 8.15 showed a two-byte call to slot 32 followed by a `-2` length after WIE incorrectly treated the read as a write. Static analysis confirms slot 31 is send and slot 32 is receive.

Phase 8.16 supplies a local three-byte command-0 server greeting to the receive path. The original client should then generate its command-1 request on slot 31, which is captured in diagnostics. There is no connection to the historical service.

Free-item fulfillment is intentionally deferred until the authentic client request/response shape is captured; this prevents corrupting inventory/save state with a guessed billing packet.
