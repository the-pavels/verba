#!/bin/bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
derived_data_path="${VERBA_DERIVED_DATA_PATH:-${TMPDIR:-/tmp}/verba-check-derived-data}"
xcode_arguments=(CODE_SIGNING_ALLOWED=NO)

case "${VERBA_SKIP_VISUAL_SNAPSHOTS:-0}" in
    0)
        ;;
    1)
        xcode_arguments+=(OTHER_SWIFT_FLAGS=-DVERBA_SKIP_VISUAL_SNAPSHOTS)
        ;;
    *)
        echo "VERBA_SKIP_VISUAL_SNAPSHOTS must be 0 or 1" >&2
        exit 1
        ;;
esac

cd "${repo_root}"

./scripts/portable-check.sh

echo "Testing the macOS host"
xcodebuild \
    -quiet \
    -project macos/Verba.xcodeproj \
    -scheme Verba \
    -configuration Debug \
    -destination "platform=macOS,arch=arm64" \
    -derivedDataPath "${derived_data_path}" \
    "${xcode_arguments[@]}" \
    test

echo "All checks passed"
