# Phase 8.22 test notes

## Inotia 1

Open `시스템 -> 캐쉬템 구매` once and wait several seconds.

Expected diagnostics:

- command-0 receive should now show `00 04` followed by payload `00 01` rather
  than the old `00 03 00` frame;
- `PHASE8_22_INOTIA1_CASH_RESPONSE_STATE` before the command-1 response should
  ideally report `value=Some(1)` from the command-0 result byte;
- command-1 receive should show a 28-byte frame beginning `00 1c 01 01`;
- if the client advances, a new `PHASE8_12_CASH_TX` command byte other than 1.

If a new error is shown, preserve the log: the native error code/state trace is
still active.

## Inotia 2

Launch normally without clearing or re-importing the title.

Expected diagnostics:

- two `PHASE8_22_INOTIA2_INSTALL_UI_SUPPRESS` lines at startup;
- `PHASE8_22_INOTIA2_EXEC_QUANTUM` reporting 4000 instructions.

The progress renderer should not appear. Internal initialization still runs.
Test title/menu animation, several skills, and multiple map transitions.
