# Phase 8.12 — Dual Inotia compatibility

## A. Inotia 2 KTF certificate-companion fallback

### Evidence from Phase 8.11

The latest Phase 8.11 device log contains:

- `[PHASE8_10_ACCESS] ... return=0xbc`
- `[PHASE8_11_EXECNAMES] ... 010100D5/010100D5.jar count=1`

The game then successfully opens/reads `cert.c2s`, followed by:

- `OPEN_REQUEST name=tcert.c2s ... exists=false packaged_len=0`
- `OPEN_RESULT ... -> -12 (NOENT)`

The supplied KTF archive has `p/cert.c2s` but no `tcert.c2s`.

### Phase 8.12 behavior

Only for AID `010100D5`, PID `PD007974` and database name `tcert.c2s`:

1. perform the normal packaged lookup;
2. if `tcert.c2s` is absent, read packaged `cert.c2s` instead;
3. feed those bytes into the normal database-open path under the requested
   `tcert.c2s` record name;
4. emit `[PHASE8_12_TCERT_ALIAS]`.

No unrelated title gets this alias.

## B. Inotia 1 cash-shop offline transport bridge

### Evidence from the cash-shop test

The test title is AID `010100D3`, PID `PD005362`. Selecting the cash-shop path
reaches:

`stub MC_netConnect(...)`

and the current generic implementation asynchronously invokes the callback with
`M_E_ERROR`. The client then closes socket 0/network and exits that connection
attempt. Therefore the existing log cannot yet contain the shop request packet
or server response contract.

### Legacy WIPI socket behavior recovered from the client

Static inspection of the client callback path is consistent with the standard
WIPI sequence:

1. `MC_netConnect`
2. `MC_netSocket(MC_AF_INET, MC_SOCKET_STREAM)`
3. `MC_netSocketConnect`
4. `MC_netSocketRead` / `MC_netSocketWrite`
5. callback registration when an operation returns `M_E_WOULDBLOCK`

The exact client branch compares the would-block result against `-19`, so the
offline read shim uses `M_E_WOULDBLOCK = -19`.

### Phase 8.12 behavior

Only for AID `010100D3`, PID `PD005362`:

- `MC_netConnect` -> asynchronous success callback;
- `MC_netSocket` -> deterministic fake fd `0`;
- `MC_netSocketConnect` -> asynchronous success callback, with no host access;
- `MC_netSocketWrite` -> accept full buffer and log up to the first 128 bytes;
- `MC_netSocketRead` -> `-19` (`M_E_WOULDBLOCK`) until a local protocol reply
  is implemented;
- `MC_netSetReadCB` / `MC_netSetWriteCB` -> accept registration;
- `MC_netSocketClose` -> success.

Important: this phase does **not** connect to `211.115.66.232` or any other old
server. It is an offline protocol-capture bridge. It should get beyond the
current immediate "network unavailable" failure and expose the original cash
shop request. Full local purchase emulation requires that newly exposed packet
contract and is intentionally deferred rather than guessed.
