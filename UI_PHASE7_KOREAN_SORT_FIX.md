# UI Phase 7 — Korean Localization + Sort Control Fix

## Sort control regression fix

The Phase 6 sort-direction implementation existed but its initializer was never called from `main()`.
That meant the arrow and home sort select had no event listeners in the running app. Phase 7 now calls
`initLibrarySortControl()` during boot and logs the initialized mode/direction.

The direction button uses `pointerup` for iOS touch responsiveness with `click` retained as a keyboard/
accessibility fallback. Direction remains persisted in global app settings.

## Korean language support

Home → Settings → Language now offers:

- English
- 한국어

The selected language persists across app restarts. The localization layer translates the WIPI Player
application UI, including the library, sorting labels, home settings, display/control settings, game action
menu, save manager controls, common editor controls, diagnostics labels, and major status text. Imported
game content is not translated or modified.

The localization is implemented through `wie_web/src/ts/i18n.ts`, so additional languages can be added
without duplicating the UI.

## Preserved from Phase 6

- Persistence-safe IndexedDB transaction handling
- Save-manager bounded scrolling
- Single-line mobile header
- Keep-screen-awake preference
- Global diagnostics
- Per-game library/display/control/save settings
