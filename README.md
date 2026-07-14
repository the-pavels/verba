# Verba

Verba is a macOS menu-bar utility for translating and proofreading text selected in another application. It uses a Rust core with a small native Swift/AppKit host for macOS integration and presentation.

Verba supports macOS 15 or later on Apple silicon. Translation uses Apple's Translation framework. Proofreading uses the OpenAI Responses API with an API key stored in the user's macOS Keychain.

- [Privacy and data handling](PRIVACY.md)
- [Release checklist](RELEASE_CHECKLIST.md)
- [Security review](SECURITY.md)
- [Release packaging](RELEASE.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)

## Development

Run the complete local check before each commit:

```sh
./scripts/check.sh
```

The check verifies Rust formatting, runs Clippy and the Rust test suite, then builds and tests the unsigned arm64 macOS Debug app. It writes Xcode DerivedData under the system temporary directory by default; set `VERBA_DERIVED_DATA_PATH` to override it.

Run the dependency security and license audit before a release:

```sh
cargo install cargo-audit --locked
cargo install cargo-deny --locked
./scripts/security-check.sh
```

The security audit uses the current RustSec advisory database and the repository policy in `deny.toml`; unlike the normal local check, it requires network access to refresh advisory data.

Build and inspect the unsigned arm64 release package with:

```sh
./scripts/package-release.sh
```

The workflow, outputs, and verification contract are documented in [Release packaging](RELEASE.md). This unsigned artifact is for inspection only.

When a Developer ID Application certificate is installed, build the signed archive with:

```sh
VERBA_DEVELOPMENT_TEAM=YOURTEAMID \
VERBA_SIGNING_IDENTITY='Developer ID Application: Your Name (YOURTEAMID)' \
./scripts/package-signed-release.sh
```

The team ID and signing identity stay outside source control. Create the distributable notarized artifact with `./scripts/notarize-release.sh`, then follow the [release checklist](RELEASE_CHECKLIST.md).

## Performance budgets

Verba emits local Points of Interest signposts under the app bundle identifier in the `Performance` category. The signposts contain request IDs, action names, presentation-state names, and budgets only. They never contain selected text, results, prompts, credentials, or error messages, and they are not sent to an analytics service.

Measure a release build with Instruments on the oldest Mac in the supported release hardware matrix. Use at least 20 cold launches and 20 invocations per action, and require the p95 measurement to meet these budgets:

| Measurement | Signposts | p95 budget |
| --- | --- | ---: |
| App initialization | `Startup` interval | 750 ms |
| Shortcut feedback | `TextAction` start to `PopupPresented` with `state=loading` | 100 ms |
| Selected-text capture | `TextAction` start to `CaptureCompleted` | 650 ms |
| Result presentation overhead | `ProcessingCompleted` to terminal `PopupPresented` | 50 ms |

The capture budget includes the bounded 500 ms synthetic-copy timeout. Translation and OpenAI provider time is intentionally outside the result-presentation budget; inspect it as the span between `CaptureCompleted` and `ProcessingCompleted` without treating network latency as UI overhead. The automated test suite verifies milestone ordering and metadata privacy, while release qualification supplies the hardware measurements required by the phase exit criterion.
