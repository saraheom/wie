# WIPI Player — TestFlight CI setup

Bundle ID:

`com.jjunnyy.wieplayer`

## Required GitHub Actions secrets

This workflow now uses **Tauri automatic iOS signing with an App Store Connect API key**.
Only these four secrets are required by `ios-testflight.yml`:

- `APPLE_DEVELOPMENT_TEAM` — your 10-character Apple Developer Team ID.
- `APPLE_API_KEY_ID` — App Store Connect Team API Key ID.
- `APPLE_API_ISSUER` — App Store Connect Issuer ID.
- `APPLE_API_PRIVATE_KEY_BASE64` — Base64 of the `AuthKey_<KEYID>.p8` private key.

The older secrets below may remain in GitHub, but the TestFlight workflow intentionally
does **not** expose them to Tauri because of a known manual provisioning-profile bug in
current Tauri/Xcode combinations:

- `IOS_CERTIFICATE`
- `IOS_CERTIFICATE_PASSWORD`
- `IOS_MOBILE_PROVISION`

## API key access

For Tauri automatic signing in CI, create the App Store Connect **Team Key with Admin access**.
This is the access level documented by Tauri for CI automatic signing. If your current key
was created with App Manager access, create a new Team Key with Admin access and replace:

- `APPLE_API_KEY_ID`
- `APPLE_API_PRIVATE_KEY_BASE64`

The Issuer ID normally stays the same.

## Build environment

The workflow uses `macos-26`, whose default Xcode is Xcode 26.x. Apple requires App Store
Connect uploads to be built with Xcode 26 or later and the iOS 26 SDK or later.

## Running

GitHub → Actions → **iOS TestFlight** → **Run workflow**.

Leave `build_number` empty unless you need to override it.

A successful run:

1. Builds the WIE WebAssembly frontend.
2. Generates the Tauri Apple project.
3. Uses the App Store Connect API key for automatic signing/provisioning.
4. Exports an App Store Connect IPA.
5. Saves the IPA as a GitHub artifact.
6. Uploads the IPA to App Store Connect for TestFlight processing.
