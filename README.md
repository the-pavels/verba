# Verba

Verba is a planned macOS menu-bar utility for translating and proofreading text selected in any application. The application will use a Rust core with a small native Swift/AppKit host for macOS integration and presentation.

## Development

Run the complete local check before each commit:

```sh
./scripts/check.sh
```

The check verifies Rust formatting, runs Clippy and the Rust test suite, then builds the unsigned arm64 macOS Debug app. It writes Xcode DerivedData under the system temporary directory by default; set `VERBA_DERIVED_DATA_PATH` to override it.
