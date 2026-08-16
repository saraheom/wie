# Phase 7.9 — Legacy Network Verification Compatibility

Phase 7.9 adds a generic compatibility layer for obsolete WIPI carrier/network
startup checks used by preserved feature-phone games.

## Root cause

WIE's previous `MC_netConnect()` implementation was a stub that always invoked
the game's callback with `M_E_ERROR` (`0xffffffff`). Games such as 바람의나라
reach their old internet/carrier verification screen and request
`MC_netConnect`; WIE then always tells the game that the network connection
failed.

Some games, such as the tested MapleStory build, tolerate that failure and
continue. Others remain blocked at the verification UI.

## Phase 7.9 behavior

`MC_netConnect()` now:

1. accepts the connection request,
2. preserves the asynchronous callback behavior,
3. waits for the same short timer delay,
4. calls the game's callback with `M_E_SUCCESS` (`0`), and
5. logs the operation as `[NET_COMPAT]`.

Expected diagnostic lines:

    [NET_COMPAT] MC_netConnect(...) -> scheduling legacy session success
    [NET_COMPAT] MC_netConnect callback status=M_E_SUCCESS(0) ...

`MC_netClose()` and `MC_netSocketClose()` are also idempotent-success operations
for this synthetic legacy network session.

## Scope

This emulates network/session availability only. It does not invent HTTP/socket
responses and does not contact historical carrier verification servers. If a
game proceeds past connection setup and then requires a real request/response,
the next missing WIPI network API will remain visible in the diagnostic log for
targeted implementation.

This behavior is global to WIPI titles rather than hard-coded to a particular
game, so other preserved games using the same obsolete carrier-connect gate can
benefit from it.

## Inotia 2

The Phase 7.8 16 MiB virtual persistent-storage change is retained. LGT Inotia 2
may progress further if its next gate is network-session availability. The KTF
Inotia 2 memory/black-screen path remains a separate issue because it has not
shown an `MC_netConnect()` call before its failure state.
