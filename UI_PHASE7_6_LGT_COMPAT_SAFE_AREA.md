# Phase 7.6 — LGT compatibility + iOS safe-area recovery

This phase converts the Phase 7.5 ABI traces into targeted LGT compatibility implementations and fixes the library header moving underneath the iOS status area after clearing diagnostics.

## LGT compatibility

### stdlib 0x416 — memcmp

The MapleStory trace calls 0x416 with two byte pointers and a size, then branches on whether the return value is zero. Phase 7.6 maps this LGT stdlib import to C `memcmp` semantics.

### stdlib 0x3f7 — sprintf

The Inotia 2 LGT trace calls 0x3f7 with an empty destination buffer, a CP949 format string `(오류번호:%d)`, and the integer error code in the first variadic argument. Phase 7.6 implements an LGT `sprintf` compatible formatter with CP949/EUC-KR strings and common `%d`, `%u`, `%x`, `%X`, `%s`, `%c`, width, zero-pad, and long/long-long handling.

### WIPIC 0x19c — MC_dbListDataBase / available database storage

LGT database services begin at 0x190. Service 0x19c is method offset 12, matching the already implemented WIPI database `list_databases` operation. Phase 7.6 wires the LGT service to that implementation.

The build keeps Phase 7.5 unknown-ABI tracing enabled, so if a game reaches another missing LGT service, the exported log will capture the next ID and call context.

## iOS safe-area recovery

The app shell now owns the viewport with a fixed inset container instead of allowing WebKit page scrolling to move it. In iPhone portrait, the library view also has a conservative 54 px top fallback when `safe-area-inset-top` is transiently reported as zero.

Closing a confirmation dialog now blurs any focused textarea/input and normalizes the document scroll position across two animation frames. This specifically protects the library header after **Logs → Clear Log**.

## Test sequence

1. Open Logs, clear the diagnostic log, close Logs, and verify the library header remains fully below the iOS status area and all top buttons are tappable.
2. MapleStory: create/load Slot 1. It should pass the former `0x416` crash. If it stops later, export the log.
3. Inotia 2 LGT (01.00.08): launch it. It should pass the former `0x3f7` crash. If it stops later, export the log.
4. Inotia 2 LGT (other revision): launch it. It should pass the former WIPIC `0x19c / 412` crash. If it stops later, export the log.
5. Original KTF Inotia 2 is still a separate compatibility track; Phase 7.6 does not claim to resolve its memory-error screen.
