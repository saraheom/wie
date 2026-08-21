# Phase 8.7 — Inotia 2 post-startup resource-base probe

This phase preserves:
- Phase 8.4 KTF packaged-resource filesystem fallback;
- Phase 8.5 / 8.6 i_pack diagnostics;
- Phase 8.1.2 Inotia 1 future-proof save-length fix.

## What Phase 8.6 proved

The `0x144E58` validation globals resolve to valid destinations, and the
validation count is zero. Therefore that routine takes its normal early-success
branch and does not dereference the zero record-base/stride values.

The crash still occurs immediately after the i_pack header parser returns.

## New narrowed path

After `0x144E58` succeeds:

    0x1450BC stores its success byte through GOT+0x1638
    0x144F48 sees that success byte and returns 1
    0x144A2C begins normal startup/resource initialization

The first data read in `0x144A2C` is equivalent to:

    kind = *(u8  *)GOT[0x0338]
    base = *(u32 *)GOT[0x0330]
    src  = base + kind * 129
    read_u16(src)

`read_u16` is guest routine `0x126088`, which copies two bytes from `src`.
If both `kind` and `base` are zero, `src` is exactly address 0 and reproduces
the observed exception.

## New markers

    [PHASE8_7]
    [INOTIA2_STARTUP_RESOURCE]

The probe records:
- the startup-success status byte;
- the selector/kind byte;
- the resource base pointer;
- the exact first computed source address;
- the four direct table pointers used next;
- code signatures for `0x1450DC` and `0x144A44`.

`predicted_null_read=true` means the static path itself predicts the exact
address-0 access seen in the runtime log.

This phase is observational only.
