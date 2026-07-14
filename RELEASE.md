# Release packaging

The release workflow produces an unsigned inspection archive, a Developer ID-signed arm64 archive, or a notarized and stapled arm64 archive. Only the final notarized artifact is intended to pass Gatekeeper without a bypass.

## Requirements

- macOS 15 or later on Apple silicon
- Xcode 16 or later with the macOS SDK selected by `xcode-select`
- the Rust toolchain from `rust-toolchain.toml`, including `aarch64-apple-darwin`

The unsigned workflow requires no signing identity or notarization credential.

The supported architecture follows `docs/adr/0002-platform-and-distribution.md`: the initial release is arm64-only. Do not add Intel output by merely changing the script. Revise the ADR and validate on an x86_64 test environment first.

## Build the package

From the repository root, run:

```sh
./scripts/package-release.sh
```

The version defaults to `[workspace.package].version` in `Cargo.toml` and the build number defaults to the current release build, `3`. Pass both explicitly when preparing a later build:

```sh
./scripts/package-release.sh 1.0.0 4
```

The requested marketing version must match the Rust workspace version. The workflow performs a Release `xcodebuild archive`, regenerates the UniFFI Swift bridge from the locked Rust build, and writes these ignored outputs under `dist/`:

- `Verba-VERSION-BUILD-arm64-unsigned.zip`
- `Verba-VERSION-BUILD-arm64-unsigned.zip.sha256`
- `Verba-VERSION-BUILD-arm64-unsigned.manifest.txt`

The manifest records the source revision and clean/dirty state plus the SHA-256 of every file in the app bundle. Only the checksum produced for the final notarized archive is publishable.

Unsigned bundle timestamps are normalized to `SOURCE_DATE_EPOCH`, which defaults to the current Git commit time. Set that variable explicitly when reproducing an unsigned artifact from exported sources that do not include Git metadata. Developer ID bundles are not modified after signing because changing signed bundle metadata invalidates the signature; Apple's secure timestamp also means signed archive bytes are not expected to be reproducible.

## Verification performed

`scripts/verify-release.sh` rejects the package unless all of the following are true:

- the app metadata contains the requested version and build number, a nonempty bundle identifier, macOS 15.0 deployment target, and menu-bar-only mode;
- the only executable architecture is arm64;
- the compiled app icon, `Contents/Resources/PrivacyInfo.xcprivacy`, and localized Accessibility/OpenAI disclosure strings are present;
- the privacy manifest declares no tracking and records the app-only UserDefaults reason `CA92.1`;
- the bundle contains exactly the executable, metadata, localization, compiled asset catalog, icon, and privacy manifest expected for item 40;
- linked libraries do not reference a developer home or Cargo target directory;
- the unsigned workflow contains no bundle signature, while the Developer ID workflow has the expected authority, team, secure timestamp, hardened-runtime flag, and no entitlement keys.

The workflow runs this verification against the archived app and again after extracting the final ZIP. The artifact is copied into `dist/` only after both checks pass.

The app uses Accessibility APIs, but macOS does not define an Info.plist usage-description key for Accessibility trust. The user-facing explanation and permission route live in Verba's localized UI. Do not add an unrelated Apple Events, Input Monitoring, or Automation usage string unless the implementation starts using the corresponding protected API.

## Updating the app icon

Edit `macos/Assets/generate-app-icon.swift`, then regenerate the checked-in raster slots:

```sh
./macos/scripts/generate-app-icon.sh
```

Run the package workflow afterward to verify that Xcode compiled `AppIcon.icns` into the bundle.

## Developer ID signing

The permanent app and Keychain service identifier is `io.github.the-pavels.verba`. Changing it after release would create a new code-signing identity, Keychain scope, and macOS privacy-permission identity.

Install a valid Developer ID Application certificate in the login Keychain, then copy its full identity and ten-character team ID from:

```sh
security find-identity -v -p codesigning
```

Supply both values for the signed workflow without writing them to the repository:

```sh
VERBA_DEVELOPMENT_TEAM=YOURTEAMID \
VERBA_SIGNING_IDENTITY='Developer ID Application: Your Name (YOURTEAMID)' \
./scripts/package-signed-release.sh
```

The workflow refuses Apple Development, Mac Distribution, ad-hoc, missing, and ambiguous identity inputs. It asks Xcode to archive with a secure timestamp, hardened runtime, and no injected base entitlements, then produces:

- `Verba-VERSION-BUILD-arm64-developer-id.zip`
- `Verba-VERSION-BUILD-arm64-developer-id.zip.sha256`
- `Verba-VERSION-BUILD-arm64-developer-id.manifest.txt`

Verba needs no hardened-runtime exception or restricted-resource entitlement. Its Rust static library is linked into the single app executable, and the bundle contains no nested framework, helper, plug-in, XPC service, or other Mach-O object. The verifier therefore requires exactly one Mach-O, verifies the app recursively with `codesign --deep --strict=all`, and checks the Developer ID authority, team, secure timestamp, hardened-runtime flag, and absence of entitlement keys.

Do not distribute the signed-only ZIP. The notarization workflow submits that exact artifact, then publishes a separately named archive after stapling changes the app bundle.

## Notarization and stapling

Create a `notarytool` Keychain profile once. Running the command without credential options keeps the Apple ID or API-key secret out of shell history and prompts for it interactively:

```sh
xcrun notarytool store-credentials verba-notary
```

The profile name is not a credential. Override the default with `VERBA_NOTARY_KEYCHAIN_PROFILE` when using another local profile or a CI Keychain. Run the complete workflow with the same external signing values used above:

```sh
VERBA_DEVELOPMENT_TEAM=YOURTEAMID \
VERBA_SIGNING_IDENTITY='Developer ID Application: Your Name (YOURTEAMID)' \
VERBA_NOTARY_KEYCHAIN_PROFILE=verba-notary \
./scripts/notarize-release.sh
```

The default service wait is 30 minutes. Set `VERBA_NOTARY_TIMEOUT` to an integer with an optional `s`, `m`, or `h` suffix when a different bound is required.

If a local interruption happens after Apple creates a submission, rerun with `VERBA_NOTARY_SUBMISSION_ID=UUID`. Recovery mode reuses the existing signed ZIP, retrieves that submission with `notarytool info`, and still requires Apple's logged SHA-256 to match the local artifact before stapling.

The workflow validates the Keychain profile before building, creates and verifies a fresh Developer ID ZIP, submits that exact ZIP with `notarytool --wait`, and retrieves the completed submission log. It rejects the result unless the submission and log are both accepted, their IDs match, and the SHA-256 in Apple's log matches the submitted ZIP. Every submission result and available log is retained under `dist/` with the submission ID, including rejected attempts.

After acceptance, the workflow extracts the submitted app, staples and validates the ticket, verifies the Developer ID signature again, and requires `spctl` to report `Notarized Developer ID`. It then creates these ignored outputs and repeats the same checks after extracting the final ZIP:

- `Verba-VERSION-BUILD-arm64-notarized.zip`
- `Verba-VERSION-BUILD-arm64-notarized.zip.sha256`
- `Verba-VERSION-BUILD-arm64-notarized.manifest.txt`
- `Verba-VERSION-BUILD-arm64-developer-id.notary-SUBMISSION_ID.result.json`
- `Verba-VERSION-BUILD-arm64-developer-id.notary-SUBMISSION_ID.log.json`

The notarized manifest records the accepted submission ID, submitted archive hash, notarization-log hash, source state, and hashes of the final stapled bundle files. Publish only the checksum generated for the notarized ZIP.

Use the [release checklist](RELEASE_CHECKLIST.md) for clean-machine installation, publication, artifact retention, and rollback. Publish [third-party notices](THIRD_PARTY_NOTICES.md) with every release.
