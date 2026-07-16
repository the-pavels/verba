#!/bin/bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
derived_data_path="${VERBA_DERIVED_DATA_PATH:-${TMPDIR:-/tmp}/verba-check-derived-data}"

cd "${repo_root}"

source_pdf="docs/macos_text_helper_development_plan.pdf"
ignore_probe="docs/adr/9999-ignore-probe.md"

echo "Checking repository hygiene"
if git ls-files --error-unmatch -- "${source_pdf}" >/dev/null 2>&1; then
    echo "Source planning PDF must not be tracked: ${source_pdf}" >&2
    exit 1
fi
if ! git check-ignore --quiet --no-index -- "${source_pdf}"; then
    echo "Source planning PDF must remain ignored: ${source_pdf}" >&2
    exit 1
fi
if git check-ignore --quiet --no-index -- "${ignore_probe}"; then
    echo "New documentation under docs/ must not be ignored: ${ignore_probe}" >&2
    exit 1
fi

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
