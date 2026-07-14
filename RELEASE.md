# Release packaging

Item 40 produces an unsigned, hardened-runtime-ready arm64 archive. Developer ID signing and notarization are separate roadmap items, so this artifact is for release inspection and as the input to the later signing workflow; it is not ready for end-user distribution.

## Requirements

- macOS 15 or later on Apple silicon
- Xcode 16 or later with the macOS SDK selected by `xcode-select`
- the Rust toolchain from `rust-toolchain.toml`, including `aarch64-apple-darwin`
- no signing identity or notarization credential

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

Bundle timestamps are normalized to `SOURCE_DATE_EPOCH`, which defaults to the current Git commit time. Set that variable explicitly when reproducing an artifact from exported sources that do not include Git metadata.

## Verification performed

`scripts/verify-release.sh` rejects the package unless all of the following are true:

- the app metadata contains the requested version and build number, a nonempty bundle identifier, macOS 15.0 deployment target, and menu-bar-only mode;
- the only executable architecture is arm64;
- the compiled app icon, `Contents/Resources/PrivacyInfo.xcprivacy`, and localized Accessibility/OpenAI disclosure strings are present;
- the privacy manifest declares no tracking and records the app-only UserDefaults reason `CA92.1`;
- the bundle contains exactly the executable, metadata, localization, compiled asset catalog, icon, and privacy manifest expected for item 40;
- linked libraries do not reference a developer home or Cargo target directory;
- the item 40 artifact remains unsigned.

The app uses Accessibility APIs, but macOS does not define an Info.plist usage-description key for Accessibility trust. The user-facing explanation and permission route live in Verba's localized UI. Do not add an unrelated Apple Events, Input Monitoring, or Automation usage string unless the implementation starts using the corresponding protected API.

## Updating the app icon

Edit `macos/Assets/generate-app-icon.swift`, then regenerate the checked-in raster slots:

```sh
./macos/scripts/generate-app-icon.sh
```

Run the package workflow afterward to verify that Xcode compiled `AppIcon.icns` into the bundle.
