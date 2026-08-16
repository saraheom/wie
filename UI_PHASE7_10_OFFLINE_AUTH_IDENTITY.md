# Phase 7.10 — Legacy Offline Authentication Identity

## Evidence from 바람의나라

The Phase 7.9 log showed no MC_netConnect / NET_COMPAT calls at the verification
gate. Instead the game queried the WIPI `ESN` system property, which WIE treated
as unsupported, and later terminated on carrier-specific LGT WIPIC service
0x266.

The uploaded LGT archive's `app_info` preserves the original OMA download URL,
including `ctn` (the handset phone number) and `device_id` (the handset model).
These are the values commercial WIPI titles historically used in first-run and
license checks.

## Changes

* Parse `ctn` and `device_id` automatically from each LGT archive's `DDurl`.
* Configure the emulator session with that per-game archived handset identity.
* `PHONENUMBER` and `MIN` return the archived CTN when available.
* `PHONEMODEL` returns the archived device_id when available.
* `ESN` no longer returns M_E_INVALID; for archived LGT apps it receives a
  deterministic numeric compatibility value derived from the preserved CTN.
* Add `[DEVICE_COMPAT]` diagnostics for identity extraction/property queries.
* Handle LGT WIPIC 0x266 as a successful legacy-network cleanup/control call
  instead of terminating WIE.

This does not fabricate remote server payloads. If a title truly requires an
extinct server's response after these local identity checks, the next trace will
show the concrete network read/HTTP API and we can emulate the minimum protocol
needed for offline preservation.
