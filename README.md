# WIPI Player

The current TestFlight workflow targets **WIPI Player 0.1.59 (Phase 8.59)**. This stabilization build preserves the verified Inotia1 global monster-reward overflow repair, save/revival compatibility, the established offline cash-shop protocol, and the existing Inotia2 compatibility/performance work.

Phase 8.58 keeps the generic LGT interface-link repair and stateful InputMethodHandler compatibility, then completes the OZ-imported `TextComponent` surface: `getMaxLength`, `setMaxLength`, `getString`, and `setString` now use real object state. `TextFieldComponent` and `TextBoxComponent` also retain their constructor text/constraint. Explicit LGT startup/error diagnostics remain enabled. The 11-record Inotia1 cash catalog is unchanged: `무기강화 주문서`, `방어구강화 주문서`, `힘의 조각`, and `마법의 가지` remain quantity **10** per purchase.

See the phase-specific notes for compatibility history and TestFlight setup.


Phase 8.58 adds the LGT platform-extension class `wec/OEMAppExecutor` required by OZ startup linking while preserving the Phase 8.55 WIPI UI compatibility fixes and all established Inotia1/Inotia2 behavior.

Phase 8.58 adds error-only LGT AOT/SVC null-fault localization for OZ after the game progressed into `base/Koablo.startApp()`.

Phase 8.58 fixes LGT Java public/imported class linking for 64-bit `long`/`double` fields. LGT class-link tables count 32-bit field words, so a `J`/`D` field is followed by a null metadata continuation slot. The linker now writes the resolved first word index and `+1` for the continuation slot instead of dereferencing address 0.

Phase 8.59 preserves the Phase 8.58 LGT wide-field repair and adds targeted class-link/vtable diagnostics for OZ `base/a` after field linking completed but the WebAssembly runtime entered a repeating stack-failure pattern.
