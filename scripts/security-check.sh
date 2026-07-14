#!/bin/bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

for tool in cargo-audit cargo-deny; do
    if ! command -v "${tool}" >/dev/null 2>&1; then
        echo "Missing ${tool}; install it with: cargo install ${tool} --locked" >&2
        exit 1
    fi
done

if ! command -v jq >/dev/null 2>&1; then
    echo "Missing jq; install it before running the security checks" >&2
    exit 1
fi

notice_check="$(mktemp "${TMPDIR:-/tmp}/verba-notices-check.XXXXXX")"
cleanup() {
    /bin/rm -f "${notice_check}"
}
trap cleanup EXIT

cd "${repo_root}"

echo "Auditing RustSec advisories"
cargo audit

echo "Checking dependency licenses, bans, and sources"
cargo deny check advisories licenses bans sources

echo "Checking third-party notices"
./scripts/generate-third-party-notices.sh "${notice_check}"
if ! /usr/bin/cmp -s THIRD_PARTY_NOTICES.md "${notice_check}"; then
    echo "THIRD_PARTY_NOTICES.md is stale; run ./scripts/generate-third-party-notices.sh" >&2
    exit 1
fi

echo "Security dependency checks passed"
