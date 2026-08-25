# WIPI Player Phase 8.35.3 — TestFlight CFBundleVersion verification correction

This is a **full-repository** Phase 8.35 build. Emulator/runtime behavior is unchanged from Phase 8.35, 8.35.1, and 8.35.2.

## Why Phase 8.35.2 failed

The complete iOS build succeeded: Rust/WASM compilation, Xcode archive, signing, IPA export, and IPA discovery all completed successfully. The workflow then unpacked the final IPA and read:

- `CFBundleShortVersionString = 0.1.35`
- `CFBundleVersion = 0.1.35.85`
- resolved GitHub Actions build number = `85`

The Phase 8.35.2 verification incorrectly required `CFBundleVersion == 85`, so it rejected an otherwise valid IPA after export.

## Phase 8.35.3 workflow correction

Tauri 2's current iOS build path composes `CFBundleVersion` as `<marketing-version>.<build-number>` (for example `0.1.35.85`). The final-IPA verification now accepts the current composed value `0.1.35.<resolved-build-number>`. It also accepts the bare numeric build number as a forward-compatible fallback in case a future Tauri release changes this behavior.

All other package identity checks remain strict:

- `CFBundleIdentifier = com.jjunnyy.wieplayer`
- `CFBundleShortVersionString = 0.1.35`
- `CFBundleVersion = 0.1.35.<resolved-build-number>` or the bare resolved build number

The artifact is named `WIPI-Player-Phase8.35.3-iOS-TestFlight`.

## Emulator behavior retained

- Inotia1 cash shop: one safe 9-record page, below the fixed 12-entry client capacity.
- `자원 교환권` is excluded from the offline shop.
- Existing `자원 교환권` use receives the offline-only failure response instead of fake network-resource success.
- Main-name recovery focuses on `자원 교환권` → `이노티아`; the secondary hero is not required for repair.
- Phase 8.34 command-123 cash-shop cleanup behavior is retained.
