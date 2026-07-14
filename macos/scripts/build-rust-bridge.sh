#!/bin/bash

set -euo pipefail

repo_root="$(cd "${SRCROOT}/.." && pwd)"
generated_dir="${SRCROOT}/Generated"
library_path="${repo_root}/target/${RUST_PROFILE}/libverba_ffi.a"

if [[ "${RUST_PROFILE}" == "release" ]]; then
    cargo build \
        --manifest-path "${repo_root}/Cargo.toml" \
        --package verba-ffi \
        --locked \
        --release
else
    cargo build \
        --manifest-path "${repo_root}/Cargo.toml" \
        --package verba-ffi \
        --locked
fi

cargo build \
    --manifest-path "${repo_root}/Cargo.toml" \
    --package uniffi-bindgen-swift \
    --locked

mkdir -p "${generated_dir}"

"${repo_root}/target/debug/uniffi-bindgen-swift" \
    --swift-sources \
    --headers \
    --modulemap \
    --module-name verba_ffiFFI \
    --modulemap-filename module.modulemap \
    "${library_path}" \
    "${generated_dir}"

/usr/bin/patch -s \
    "${generated_dir}/verba_ffi.swift" \
    "${SRCROOT}/scripts/uniffi-swift-6-sendable.patch"
