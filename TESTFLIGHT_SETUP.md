# WIPI Player TestFlight CI setup

Bundle ID: `com.jjunnyy.wieplayer`

The workflow `.github/workflows/ios-testflight.yml` expects these GitHub Actions repository secrets:

- `IOS_CERTIFICATE` — Base64 encoded Apple Distribution `.p12`
- `IOS_CERTIFICATE_PASSWORD` — password used to export the `.p12`
- `IOS_MOBILE_PROVISION` — Base64 encoded App Store Connect provisioning profile for `com.jjunnyy.wieplayer`
- `APPLE_DEVELOPMENT_TEAM` — Apple Developer Team ID
- `APPLE_API_KEY_ID` — App Store Connect API key ID
- `APPLE_API_ISSUER` — App Store Connect API issuer ID
- `APPLE_API_PRIVATE_KEY_BASE64` — Base64 encoded App Store Connect `.p8` key

Run **Actions → iOS TestFlight → Run workflow**. The workflow builds an App Store Connect IPA and uploads it to App Store Connect.
