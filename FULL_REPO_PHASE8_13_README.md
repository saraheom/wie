# WIPI Player full repository — Phase 8.13

This is a complete repository snapshot based on Phase 8.12. It preserves all
prior WIPI Player UI, save, MapleStory, Inotia 1, and Inotia 2 work while
replacing the two Phase 8.12 boundaries identified by the latest iPhone logs.

## Inotia 2

Phase 8.12's `tcert.c2s <- cert.c2s` alias is removed. The exact legacy
certificate validator used by AID `010100D5` / PID `PD007974` is now
byte-guarded and made to return success, allowing the original client to follow
its normal post-validation branch.

Marker:

`[PHASE8_13_INOTIA2_CERT_BYPASS]`

## Inotia 1 cash shop

The KTF network method table is extended from 30 to 31 entries. The new slot 30
implements the carrier-extension socket-connect callback path used by AID
`010100D3` / PID `PD005362`, which previously read beyond WIE's method table
and crashed immediately after `MC_netSocket`.

Markers:

- `[PHASE8_13_INOTIA1_NET30]`
- `[PHASE8_13_INOTIA1_NET30_CB]`

The offline bridge remains local-only; it does not contact the original game
servers. Existing packet capture remains enabled as `[PHASE8_12_CASH_TX]`.

## Build hygiene

The TestFlight workflow performs a clean WASM rebuild, verifies Phase 8.9,
8.10, 8.11, 8.12 and 8.13 markers, verifies the new slot-30 method is selected,
and explicitly fails if the obsolete Phase 8.12 `tcert.c2s` alias remains in
the database implementation.
