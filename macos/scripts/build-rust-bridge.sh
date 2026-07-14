#!/bin/bash

set -euo pipefail

repo_root="$(cd "${SRCROOT}/.." && pwd)"
generated_dir="${SRCROOT}/Generated"
rust_profile="${RUST_PROFILE:?RUST_PROFILE is required}"
requested_archs="${VERBA_RUST_ARCHS:-${ARCHS:-arm64}}"
rust_target="aarch64-apple-darwin"

if [[ " ${requested_archs} " != " arm64 " ]]; then
    echo "Unsupported release architecture list: ${requested_archs}" >&2
    echo "Verba currently supports arm64 only; revise ADR 0002 before adding another architecture." >&2
    exit 1
fi

if ! rustup target list --installed | /usr/bin/grep -Fxq "${rust_target}"; then
    echo "Missing Rust target ${rust_target}; install it with: rustup target add ${rust_target}" >&2
    exit 1
fi

cargo_args=(
    build
    --manifest-path "${repo_root}/Cargo.toml"
    --package verba-ffi
    --locked
    --target "${rust_target}"
)

if [[ "${rust_profile}" == "release" ]]; then
    cargo_args+=(--release)
elif [[ "${rust_profile}" != "debug" ]]; then
    echo "Unsupported Rust profile: ${rust_profile}" >&2
    exit 1
fi

cargo "${cargo_args[@]}"

cargo build \
    --manifest-path "${repo_root}/Cargo.toml" \
    --package uniffi-bindgen-swift \
    --locked

thin_library="${repo_root}/target/${rust_target}/${rust_profile}/libverba_ffi.a"
xcode_library_dir="${repo_root}/target/verba-xcode/${rust_profile}"
xcode_library="${xcode_library_dir}/libverba_ffi.a"
staging_dir="$(mktemp -d "${TMPDIR:-/tmp}/verba-bindings.XXXXXX")"
trap '/bin/rm -rf "${staging_dir}"' EXIT

mkdir -p "${generated_dir}" "${xcode_library_dir}"
/usr/bin/install -m 0644 "${thin_library}" "${xcode_library}"

"${repo_root}/target/debug/uniffi-bindgen-swift" \
    --swift-sources \
    --headers \
    --modulemap \
    --module-name verba_ffiFFI \
    --modulemap-filename module.modulemap \
    "${thin_library}" \
    "${staging_dir}"

/usr/bin/patch -s \
    "${staging_dir}/verba_ffi.swift" \
    "${SRCROOT}/scripts/uniffi-swift-6-sendable.patch"

for generated_file in module.modulemap verba_ffi.swift verba_ffiFFI.h; do
    /usr/bin/install -m 0644 \
        "${staging_dir}/${generated_file}" \
        "${generated_dir}/${generated_file}"
done
