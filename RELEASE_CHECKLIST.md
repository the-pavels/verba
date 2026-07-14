# Release checklist

Use this checklist for every direct-distribution Verba release. The authoritative build and notarization commands are in [RELEASE.md](RELEASE.md).

## 1. Freeze the candidate

- [ ] Choose a semantic version and monotonically increasing build number. Update the Cargo workspace version and user-facing release notes.
- [ ] Start from a clean, reviewed source revision and record its commit and tag.
- [ ] Confirm the supported matrix remains macOS 15 or later on Apple silicon. Record any intentional compatibility change in an ADR and the release notes.
- [ ] Review [PRIVACY.md](PRIVACY.md), [SECURITY.md](SECURITY.md), and the app's first-use disclosures against the current implementation.
- [ ] Regenerate [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) with `./scripts/generate-third-party-notices.sh` and confirm `git diff --exit-code -- THIRD_PARTY_NOTICES.md` is clean.

## 2. Qualify the source

- [ ] Run `./scripts/check.sh`.
- [ ] Install `cargo-audit` and `cargo-deny` if needed, then run `./scripts/security-check.sh` with current network access.
- [ ] Review all dependency advisories, license obligations, duplicate-version warnings, and source-policy output. Do not waive a new result silently.
- [ ] Measure the release build against the performance budgets in [README.md](README.md) on the oldest supported Mac.
- [ ] Complete the manual application matrix in the development plan, including TextEdit, Notes, Safari, Mail, a Chromium/Electron app, secure fields, clipboard races, denied Accessibility, offline OpenAI, invalid credentials, oversized input, and repeated shortcut invocation.

## 3. Build, sign, and notarize

- [ ] Confirm the Developer ID Application certificate and private key are available only on the controlled release machine.
- [ ] Confirm `xcrun notarytool history --keychain-profile verba-notary` authenticates without placing credentials in the repository or shell history.
- [ ] Export `VERBA_DEVELOPMENT_TEAM` and `VERBA_SIGNING_IDENTITY`, then run `./scripts/notarize-release.sh VERSION BUILD`.
- [ ] Require an accepted notarization result, matching submitted-archive hash, valid stapled ticket, valid Developer ID signature, expected team, hardened runtime without exception entitlements, and a successful Gatekeeper assessment.
- [ ] Require the notarized manifest to report the frozen source revision and `source-state=clean`.

## 4. Test installation on a clean Mac

- [ ] Copy only the candidate notarized ZIP and its `.sha256` file to an Apple-silicon Mac that has never installed this Verba bundle identifier.
- [ ] From the download directory, run `shasum -a 256 -c Verba-VERSION-BUILD-arm64-notarized.zip.sha256` and require `OK`.
- [ ] Extract the ZIP, move `Verba.app` to `/Applications`, and open it normally from Finder. Do not use a Gatekeeper bypass or remove quarantine metadata.
- [ ] Confirm Verba appears only in the menu bar and that About reports the expected version and build.
- [ ] Confirm the Accessibility explanation appears before the system prompt, denial is recoverable, and approval enables selection capture after returning from System Settings.
- [ ] Configure both shortcuts, a target language, and an OpenAI API key. Quit and reopen Verba; confirm settings and Keychain state persist.
- [ ] Translate and proofread selected text in at least two application families. Confirm clipboard restoration, copy-result behavior, no automatic replacement, and actionable offline/permission/key errors.
- [ ] Follow the complete cleanup steps in [PRIVACY.md](PRIVACY.md), reinstall, and confirm Verba starts with default settings, no API key, and no retained Accessibility grant.

## 5. Publish

- [ ] Publish the exact `Verba-VERSION-BUILD-arm64-notarized.zip` that passed clean-machine testing. Never re-zip or modify it after qualification.
- [ ] Publish its generated `.sha256` file beside it and repeat the checksum verification after downloading both public assets.
- [ ] Publish `THIRD_PARTY_NOTICES.md`, release notes, supported OS/CPU requirements, known limitations, installation steps, [PRIVACY.md](PRIVACY.md), and [SECURITY.md](SECURITY.md).
- [ ] Keep signed-only and unsigned archives private and label them as non-distributable.

## 6. Retain release evidence

Retain the public notarized ZIP, checksum, notices, release notes, source tag/commit, manifest, notarization result, and notarization log indefinitely. They are the evidence needed to reproduce provenance and verify an old download.

Keep the matching Xcode archive and dSYM in access-controlled storage for as long as the release is supported and for at least one additional year. Treat dSYMs and crash artifacts as sensitive. Never archive the Developer ID private key, notary credential, OpenAI key, or other secrets with release artifacts.

Record the person, date, source revision, build machine/Xcode version, notarization submission ID, public asset URLs, and final SHA-256 in the release record.

## 7. Roll back or withdraw

Verba has no automatic updater. A rollback is therefore a release-publication and user-installation operation, not a server-side switch.

- Stop promoting the affected version and clearly mark it withdrawn without deleting its provenance record.
- Restore links to the last qualified notarized ZIP and its original checksum, notices, privacy document, and release notes. Never rebuild an old version under the same version/build identifier.
- Tell affected users to quit Verba, replace the application in `/Applications` with the previous qualified build, and reopen it. Preferences and the Keychain item use the permanent bundle/service identifier and normally survive replacement.
- If persisted settings or credentials are implicated, direct users through the complete cleanup steps in [PRIVACY.md](PRIVACY.md) before installing the previous build.
- Prepare a corrected release with a new version or build number, repeat this entire checklist, and document the reason for withdrawal.
