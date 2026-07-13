#!/bin/bash

set -euo pipefail

repo_root="$(cd "${SRCROOT}/.." && pwd)"
generated_dir="${SRCROOT}/Generated"
library_path="${repo_root}/target/${RUST_PROFILE}/libverba_ffi.a"

if [[ "${RUST_PROFILE}" == "release" ]]; then
    cargo build \
        --manifest-path "${repo_root}/Cargo.toml" \
        --package verba-ffi \
        --release
else
    cargo build \
        --manifest-path "${repo_root}/Cargo.toml" \
        --package verba-ffi
fi

cargo build \
    --manifest-path "${repo_root}/Cargo.toml" \
    --package uniffi-bindgen-swift

mkdir -p "${generated_dir}"

"${repo_root}/target/debug/uniffi-bindgen-swift" \
    --swift-sources \
    --headers \
    --modulemap \
    --module-name verba_ffiFFI \
    --modulemap-filename module.modulemap \
    "${library_path}" \
    "${generated_dir}"
