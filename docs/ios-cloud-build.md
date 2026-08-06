# iOS cloud build

The first iOS milestone builds **WIPI Player** for the Apple-silicon iPhone Simulator. This validates the complete pipeline without requiring an Apple Developer certificate:

1. Compile `wie_web` to WebAssembly.
2. Bundle the frontend into the Tauri application.
3. Generate Tauri's Apple/Xcode project.
4. Compile the Rust mobile library for `aarch64-apple-ios-sim`.
5. Build and upload a zipped `.app` simulator bundle.

## Run the workflow

1. Push this project to the GitHub fork.
2. Open **Actions** in GitHub.
3. Select **iOS Simulator Smoke Build**.
4. Choose **Run workflow**.
5. After the job succeeds, download the `WIPI-Player-iOS-Simulator` artifact.

The simulator artifact cannot be installed on a physical iPhone. Its purpose is to prove that the source, WebAssembly frontend, Tauri mobile layer, Rust target, and generated Xcode project compile together.

## Next milestone: device IPA

After the simulator build passes, add Apple signing to a separate device workflow. That workflow will require:

- an Apple Developer Program membership;
- a registered App ID matching `com.saraheom.wieplayer`;
- an Apple Distribution or Apple Development certificate exported as `.p12`;
- a matching provisioning profile;
- GitHub Actions secrets containing the certificate, profile, and passwords.

Do not commit certificates, provisioning profiles, private keys, or passwords to the repository.

## Relevant commands

From the repository root on macOS:

```bash
npm ci
npm run build:prod
cd wie_app
cargo tauri ios init --ci
cargo tauri ios build --target aarch64-sim --ci --config '{"build":{"beforeBuildCommand":""}}'
```

The explicit empty `beforeBuildCommand` prevents rebuilding the WebAssembly frontend a second time during the Tauri build.
