# Release packaging

The release workflow produces either an unsigned inspection archive or a Developer ID-signed arm64 archive. Notarization and stapling remain a separate roadmap item, so neither artifact is ready for end-user distribution yet.

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

The version defaults to `[workspace.package].version` in `Cargo.toml` and the build number defaults to `1`. Pass both explicitly when preparing a later build:

```sh
./scripts/package-release.sh 0.1.0 2
```

The requested marketing version must match the Rust workspace version. The workflow performs a Release `xcodebuild archive`, regenerates the UniFFI Swift bridge from the locked Rust build, and writes these ignored outputs under `dist/`:

- `Verba-VERSION-BUILD-arm64-unsigned.zip`
- `Verba-VERSION-BUILD-arm64-unsigned.zip.sha256`
- `Verba-VERSION-BUILD-arm64-unsigned.manifest.txt`

The manifest records the source revision and clean/dirty state plus the SHA-256 of every file in the app bundle. The archive checksum is the value to publish after signing and notarization are added; item 42 will need to regenerate it after those operations change the bundle.

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

Do not submit or distribute this signed ZIP yet. Item 42 will notarize the exact signed artifact, staple the accepted ticket, validate it with Gatekeeper, and publish a new checksum.
