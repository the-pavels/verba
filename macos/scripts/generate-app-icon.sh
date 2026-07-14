#!/bin/bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source_file="${repo_root}/macos/Assets/generate-app-icon.swift"
output_dir="${repo_root}/macos/Verba/Assets.xcassets/AppIcon.appiconset"
module_cache="${repo_root}/target/swift-module-cache"

mkdir -p "${output_dir}" "${module_cache}"
xcrun swift -module-cache-path "${module_cache}" "${source_file}" "${output_dir}"
