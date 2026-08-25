# WIPI Player Phase 8.35.2 — TestFlight CI verification correction

This is a **full-repository** Phase 8.35 build. Runtime behavior is unchanged from Phase 8.35 / 8.35.1.

## Why Phase 8.35.1 failed

The iOS build, signing, archive, and IPA export all succeeded. The workflow then looked for `phase8_35_build.txt` under `wie_app/gen/apple/assets`. Tauri does not expose `frontendDist` there as loose files; those assets are embedded during the application build. The check therefore failed even though the IPA had already been produced.

## Phase 8.35.2 workflow

Before the iOS build, the workflow still forces a clean WASM build and verifies `PHASE8_35_RUNTIME_SENTINEL` in both `wie_web/pkg/wie_web_bg.wasm` and the Webpack-emitted `.wasm`. It also deletes `wie_app/gen/apple` before regenerating the Apple project.

After IPA export, the workflow no longer guesses a Tauri asset directory. It unpacks the final IPA and verifies the actual app `Info.plist` values:

- `CFBundleIdentifier = com.jjunnyy.wieplayer`
- `CFBundleShortVersionString = 0.1.35`
- `CFBundleVersion =` the resolved GitHub Actions build number

The IPA is then uploaded as the `WIPI-Player-Phase8.35.2-iOS-TestFlight` artifact and sent to App Store Connect.

## Emulator behavior retained

- Inotia1 cash shop: one safe 9-record page, below the fixed 12-entry client capacity.
- `자원 교환권` is excluded from the offline shop.
- Existing `자원 교환권` use receives the offline-only failure response instead of fake network-resource success.
- Main-name recovery focuses on `자원 교환권` → `이노티아`; the secondary hero is not required for repair.
- Phase 8.34 command-123 cash-shop cleanup behavior is retained.
