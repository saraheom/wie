# Phase 8.13 — Dual Inotia compatibility

## A. Inotia 2 KTF — bypass obsolete carrier certificate validator

### Evidence from the Phase 8.12 device log

The Phase 8.12 run confirms the earlier compatibility gates are active:

- `[PHASE8_10_ACCESS] ... return=0xbc`
- `[PHASE8_11_EXECNAMES] ... 010100D5/010100D5.jar count=1`
- `[PHASE8_12_TCERT_ALIAS] ... bytes=23`

After the aliased `tcert.c2s` record is opened and closed, the title makes no
further observable WIPI calls before the user exits. This shows that treating
`cert.c2s` as a valid `tcert.c2s` companion does not reproduce the legacy
carrier certificate semantics.

### Native control-flow finding

For the exact KTF title AID `010100D5`, PID `PD007974`, native image length
608192 bytes:

- guest `0x0012ae44` is the legacy certificate validator;
- the caller at `0x00176ab6` tests its return value;
- nonzero branches directly to the normal success continuation at
  `0x00176982`;
- zero enters the authentication-error path.

Phase 8.13 therefore removes the Phase 8.12 `tcert.c2s` alias and, only for
this exact title/image shape, byte-guards the validator entry
`f0 b5 57 46` and replaces it with:

`01 20 70 47` = `movs r0,#1; bx lr`

Marker:

`[PHASE8_13_INOTIA2_CERT_BYPASS]`

This does not fabricate a certificate, contact a historical server, or alter
other WIPI titles.

## B. Inotia 1 KTF — missing carrier network slot 30

### Evidence from the Phase 8.12 cash-shop device log

The offline bearer callback succeeds and the client reaches:

- `[PHASE8_12_INOTIA1_NET] ... callback success`
- `[INOTIA1_CASH_NET] MC_netSocket domain=2 type=1 -> fd=0`

The WASM runtime then faults immediately, before `MC_netSocketConnect`,
`[PHASE8_12_CASH_TX]`, or any server-protocol packet appears.

### Native ABI finding

Static inspection of PD005362's success callback shows that immediately after
creating the socket, it loads a function pointer from network-interface offset
`0x78`. Since WIPI method-table entries are 4-byte pointers, this is **slot
30**.

The Phase 8.12 WIE network table contains slots 0..29 only. The guest was
therefore reading one function pointer past the allocated table, explaining the
immediate WASM fault.

The call site passes six arguments consistent with the KTF carrier-extension
shape:

`(fd, host, port, flags, callback, callback_param)`

and treats direct return `0` or `M_E_WOULDBLOCK (-19)` as non-failure.

### Phase 8.13 behavior

The KTF network table now includes slot 30. For only AID `010100D3`, PID
`PD005362`, that method:

1. logs all six recovered arguments;
2. returns synchronous success (`0`);
3. if the callback is a valid Thumb pointer within the known Inotia 1 native
   image, schedules a conservative async success callback with status `0`;
4. otherwise suppresses the jump and leaves a diagnostic instead of executing
   an untrusted pointer.

Markers:

- `[PHASE8_13_INOTIA1_NET30]`
- `[PHASE8_13_INOTIA1_NET30_CB]`

The existing Phase 8.12 write/read instrumentation remains in place. If this
gets far enough to transmit the original cash-shop request, the packet begins
appearing under `[PHASE8_12_CASH_TX]`. No historical Com2uS host is contacted.

## Expected next test

### Inotia 2

Launch normally. The log should show
`[PHASE8_13_INOTIA2_CERT_BYPASS]` early during native load. The former
`[PHASE8_12_TCERT_ALIAS]` marker must not appear. Record what screen or next
compatibility boundary follows.

### Inotia 1

Open `시스템 -> 캐쉬템 구매`. The log should show
`[PHASE8_13_INOTIA1_NET30]`; normally it should also show the callback marker.
If `[PHASE8_12_CASH_TX]` appears, keep the cash-shop screen open for several
seconds before exporting diagnostics so the complete request/receive sequence
is captured.
