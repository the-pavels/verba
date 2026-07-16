#!/bin/bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
derived_data_path="${VERBA_DERIVED_DATA_PATH:-${TMPDIR:-/tmp}/verba-check-derived-data}"

cd "${repo_root}"

echo "Checking Rust formatting"
cargo fmt --all -- --check

echo "Linting Rust"
cargo clippy --locked --workspace --all-targets -- -D warnings

echo "Testing Rust"
cargo test --locked --workspace

echo "Testing the macOS host"
xcodebuild \
    -quiet \
    -project macos/Verba.xcodeproj \
    -scheme Verba \
    -configuration Debug \
    -destination "platform=macOS,arch=arm64" \
    -derivedDataPath "${derived_data_path}" \
    CODE_SIGNING_ALLOWED=NO \
    test

echo "All checks passed"
