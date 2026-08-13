# Building a signed iPhone IPA with GitHub Actions

This repository includes `.github/workflows/ios-device-adhoc.yml`, a manual GitHub Actions workflow that builds a signed **Ad Hoc** IPA for physical iPhones.

The app bundle identifier is:

`com.jjunnyy.wieplayer`

## Apple Developer prerequisites

Before the workflow can build an installable IPA:

1. Enroll in the Apple Developer Program.
2. Register `com.jjunnyy.wieplayer` as an App ID / Identifier.
3. Register every iPhone that should be able to install the Ad Hoc build by UDID.
4. Create an **Apple Distribution** signing certificate and retain its private key.
5. Export the certificate + private key as a password-protected `.p12` file.
6. Create an **Ad Hoc** provisioning profile for `com.jjunnyy.wieplayer` using the distribution certificate and the registered test iPhone(s).
7. Download the `.mobileprovision` profile.

## GitHub repository secrets

Open the repository in GitHub and go to:

**Settings → Secrets and variables → Actions → New repository secret**

Create these four secrets:

| Secret | Value |
| --- | --- |
| `IOS_CERTIFICATE` | Base64 representation of the `.p12` file |
| `IOS_CERTIFICATE_PASSWORD` | Password used when exporting the `.p12` file |
| `IOS_MOBILE_PROVISION` | Base64 representation of the Ad Hoc `.mobileprovision` file |
| `APPLE_DEVELOPMENT_TEAM` | Apple Developer Team ID (usually 10 characters) |

Do not commit any certificate, private key, provisioning profile, or password to the repository.

### Base64 on Windows PowerShell

Convert the `.p12` file:

```powershell
[Convert]::ToBase64String([IO.File]::ReadAllBytes("C:\path\WIPIPlayer.p12")) | Set-Clipboard
```

Convert the `.mobileprovision` file:

```powershell
[Convert]::ToBase64String([IO.File]::ReadAllBytes("C:\path\WIPIPlayer.mobileprovision")) | Set-Clipboard
```

Paste each clipboard result into the matching GitHub Actions secret.

## Run the workflow

Go to:

**Actions → iOS Device Ad Hoc IPA → Run workflow**

Leave the build-number field blank unless you specifically want to set it. The workflow otherwise uses the GitHub run number.

A successful run uploads an artifact named:

`WIPI-Player-iOS-AdHoc`

It contains:

`WIPI-Player-iOS-AdHoc.ipa`

Only iPhones whose UDIDs were included in the Ad Hoc provisioning profile can install that IPA.

## Updating the provisioning profile

When adding another iPhone:

1. Register its UDID in Apple Developer.
2. Regenerate/download the Ad Hoc provisioning profile.
3. Replace the `IOS_MOBILE_PROVISION` GitHub secret with the Base64 encoding of the new profile.
4. Run the workflow again.
