# Phase 7.7 — MapleStory GetContext + UI Cleanup

## UI cleanup
The old Experimental State Lab markup was accidentally appended after the closing `</html>` tag. WKWebView could hoist that malformed trailing DOM above the library header. Phase 7.7 removes the State Lab markup from the app UI completely.

## LGT compatibility
Phase 7.6 fixed stdlib 0x416 (`memcmp`). MapleStory then advanced to WIPIC SVC 0xCF (207). The service lies directly after `SetContext` (0xCE), and its traced arguments match the inverse graphics-context operation: a context pointer, context-field selector, and output pointer.

Phase 7.7 adds `GetContext = 0xCF` and implements `MC_grpGetContext` as the inverse of the existing `MC_grpSetContext`, including clip rectangle, colors, alpha, font/style, callback/parameter, and offset fields.

The existing unknown-call ABI tracer remains enabled for the next missing service.

## Inotia 2
The LGT Inotia 2 build now gets past stdlib 0x3F7 and successfully formats its error text. The KTF builds still exhibit the independent memory/black-screen behavior and are not altered by this graphics compatibility change.
