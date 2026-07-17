# Continuous verification

GitHub Actions runs two read-only workflows. Neither workflow has access to release credentials, the developer Keychain, signing identities, notarization profiles, or OpenAI credentials.

## Quality workflow

`.github/workflows/quality.yml` runs for every pull request targeting `main`, every push to `main`, and manual dispatches. Superseded runs on the same pull request or branch are cancelled.

- **Portable Rust and repository checks** runs on `ubuntu-24.04`. Its local equivalent is:

  ```bash
  ./scripts/portable-check.sh
  ```

- **macOS host** runs on the Apple-silicon `macos-15` runner with `DEVELOPER_DIR` pinned to Xcode 16.4. It builds the Rust/Swift bridge and runs the complete Rust and Xcode test suites. Its local equivalent is:

  ```bash
  ./scripts/check.sh
  ```

The workflows cache only downloaded Cargo registry and Git dependency data. They do not cache compiled targets, Xcode DerivedData, generated bindings, credentials, or release artifacts.

Configure the `main` branch ruleset to require these two Quality checks before merge:

- `Portable Rust and repository checks`
- `macOS host (macOS 15, Xcode 16.4)`

Do not make the path-filtered Security workflow a universal required check; it is required operationally when dependency inputs change and otherwise runs on its weekly schedule. A cancelled superseded Quality run is not a successful check—the replacement run must complete before the branch ruleset permits merging.

## Security workflow

`.github/workflows/security.yml` runs when dependency policy, Rust or Swift lockfiles, third-party notices, or the security scripts change. It also runs every Monday at 05:17 UTC and can be dispatched manually, ensuring advisory data is refreshed even when lockfiles do not change.

The workflow installs the same pinned `cargo-audit` and `cargo-deny` versions used for release verification, then runs:

```bash
cargo install cargo-audit --version 0.22.2 --locked
cargo install cargo-deny --version 0.20.2 --locked
./scripts/security-check.sh
```

## Protected release operations

Live OpenAI evaluation, Developer ID signing, notarization, update-feed signing, and publication remain explicit owner-run release steps. They are not triggered by pull requests or ordinary pushes and are not granted secrets by these workflows.
