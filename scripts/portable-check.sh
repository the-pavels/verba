#!/bin/bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "${repo_root}"

echo "Checking repository hygiene"
for ignored_path in \
    docs/ci-ignore-probe.md \
    docs/macos_text_helper_development_plan.pdf; do
    if ! git check-ignore -q "${ignored_path}"; then
        echo "Expected ${ignored_path} to remain ignored" >&2
        exit 1
    fi
done

if git ls-files --error-unmatch docs/macos_text_helper_development_plan.pdf >/dev/null 2>&1; then
    echo "The local source PDF must not be tracked" >&2
    exit 1
fi

tracked_generated="$(
    git ls-files \
        | grep -E '^(target|dist|DerivedData|macos/Generated)/|(^|/)xcuserdata/|\.xcuserstate$|\.dSYM(/|$)|\.app(/|$)' \
        || true
)"
if [[ -n "${tracked_generated}" ]]; then
    echo "Generated build output must not be tracked:" >&2
    echo "${tracked_generated}" >&2
    exit 1
fi

echo "Checking shell syntax"
while IFS= read -r script; do
    /bin/bash -n "${script}"
done < <(git ls-files -- ':(glob)**/*.sh')

echo "Checking Rust formatting"
cargo fmt --all -- --check

echo "Linting Rust"
cargo clippy --locked --workspace --all-targets -- -D warnings

echo "Testing Rust"
cargo test --locked --workspace

echo "Portable checks passed"
