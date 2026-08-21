# Phase 8.8 — Inotia 2 KTF stream-seek semantics fix

This phase preserves:
- Phase 8.4 KTF packaged-resource filesystem fallback;
- Phase 8.5–8.7 Inotia 2 diagnostics;
- Phase 8.1.2 Inotia 1 future-proof save-length fix.

## What Phase 8.7 proved

Immediately before the deterministic address-0 crash:

    startup status = 1
    resource kind  = 0
    resource base  = 0
    first source   = 0

The native path therefore predicts the exact null read.

## Root cause upstream

Static analysis of `client.bin1149832` shows that resource ID 0x43 is normally
initialized by `0x101950 -> 0x1432F0`.

Before that table can be initialized, `0x14368C` must load and parse
`game.dat`.

The helper at `0x122FA8` calculates stream length using slot 4 as a normal
seek primitive:

    saved = seek(handle, 0, CUR)
    begin = seek(handle, 0, SET)
    end   = seek(handle, 0, END)
    seek(handle, saved, SET)
    length = end - begin

The prior WIE KTF implementation treated modes 0, 1 and 2 as the same absolute
rewind and returned 0. For a 40,267-byte game.dat this made the native helper
report length 0, causing the game.dat resource loader to fail before the
resource globals were initialized.

## Phase 8.8 behavior

For PID PD007974 only, KTF slot 4 now implements:

    mode 0 = SEEK_SET
    mode 1 = SEEK_CUR
    mode 2 = SEEK_END

and returns the resulting stream position.

For the observed game.dat sequence we expect:

    seek(0, CUR) -> 0
    seek(0, SET) -> 0
    seek(0, END) -> 40267
    seek(0, SET) -> 0

so `0x122FA8` returns 40267.

This is title-scoped for the first device test. Other KTF titles retain their
existing behavior, including Inotia 1's specialized save-length compatibility
logic.

## New markers

    [PHASE8_8]
    [INOTIA2_SEEK_FIX]
    [INOTIA2_RESOURCE_INIT]

A successful causal-chain result should show:
- game.dat mode=2 returning position 40267;
- a large game.dat read after the first byte;
- nonzero `game_source`;
- resource ID 0x43 kind/base initialized;
- `predicted_null_read=false`;
- execution advances beyond the old address-0 crash.

No network behavior or ARM binary bytes are modified.
