# Phase 7.12 — Inotia 2 LGT Certificate Compatibility

This phase is based on Phase 7.11, where MapleStory is confirmed working again.

## Diagnosis

The one-shot Inotia trace shows LGT Inotia 2 enters its own error 3100 path
immediately after startup thread 2 begins. Static inspection of both uploaded
LGT `binary.mod` revisions identifies the obsolete carrier/download certificate
validator that reads `cert.c2s`.

This is not a global network or memory workaround.

## Compatibility shim

Only AID `0002BA13`, PID `PD132645` is eligible.

Two known binaries are recognized using:
- exact `binary.mod` length
- exact original Thumb prologue at the validator
- AID/PID match

01.00.08:
- validator guest address: `0x21b4`
- accepted return convention: `0`

01.00.04:
- validator guest address: `0x7ca3c`
- accepted return convention: `1`

The patch is applied only to the in-memory copy of `binary.mod` before WIE loads
the ELF sections. Imported ZIP files and library files are not modified.

Expected diagnostic:
`[INOTIA_COMPAT] Inotia 2 LGT <version>: bypassed obsolete cert.c2s validation ...`

If a different Inotia 2 binary is encountered, the shim is not applied and the
log reports that the binary revision is unsupported.

## Regression safety

Phase 7.11 MapleStory behavior is unchanged. No `MC_netConnect`, device identity,
save persistence, graphics, or general WIPI ABI behavior is changed in this phase.
