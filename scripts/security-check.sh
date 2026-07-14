#!/bin/bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

for tool in cargo-audit cargo-deny; do
    if ! command -v "${tool}" >/dev/null 2>&1; then
        echo "Missing ${tool}; install it with: cargo install ${tool} --locked" >&2
        exit 1
    fi
done

cd "${repo_root}"

echo "Auditing RustSec advisories"
cargo audit

echo "Checking dependency licenses, bans, and sources"
cargo deny check advisories licenses bans sources

echo "Security dependency checks passed"
