# WIPI Player full repository — Phase 8.12

This is a complete repository snapshot based on Phase 8.11. It keeps all prior
WIPI Player UI, save, MapleStory, Inotia 1 and Inotia 2 compatibility work and
adds two narrowly scoped legacy-title compatibility paths.

## Inotia 2 KTF — missing `tcert.c2s`

Phase 8.11 is confirmed active in the latest device log: the game accepts both
the Phase 8.10 `0xBC` access mask and the Phase 8.11
`010100D5/010100D5.jar` executable registration. The next observable boundary
is an open of `tcert.c2s`, which returns `M_E_NOENT` because the preserved game
archive contains `p/cert.c2s` but no `tcert.c2s`.

For only AID `010100D5`, PID `PD007974`, Phase 8.12 exposes the packaged
`cert.c2s` bytes when the guest asks for missing `tcert.c2s`. The marker is:

`[PHASE8_12_TCERT_ALIAS]`

## Inotia 1 KTF — cash-shop offline network bridge

The cash-shop test reaches `MC_netConnect`. WIE's generic network stub reports
failure through the callback, so the original client never reaches its socket
or shop protocol code.

For only AID `010100D3`, PID `PD005362`, Phase 8.12 adds an offline transport
bridge. It never contacts the historical game server. It reports bearer and
TCP-connect success, supplies a fake local socket, accepts outbound writes,
returns WIPI `M_E_WOULDBLOCK` (`-19`) while no local response exists, and
accepts read/write callback registration.

Outbound client packets are logged with:

`[PHASE8_12_CASH_TX]`

and the idle receive boundary with:

`[PHASE8_12_CASH_RX_WAIT]`

This intentionally recovers the original client's next protocol request before
we invent any server reply. A later phase can implement a local/offline shop
response and transaction only after the real packet format is visible in a
device diagnostic log.

## Build hygiene

The TestFlight workflow still forces a clean WIE WASM rebuild and verifies all
Phase 8.9 through Phase 8.12 source markers before packaging.
