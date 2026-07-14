#!/bin/bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output_path="${1:-${repo_root}/THIRD_PARTY_NOTICES.md}"

if ! command -v jq >/dev/null 2>&1; then
    echo "Missing jq; install it before generating third-party notices" >&2
    exit 1
fi

work_dir="$(mktemp -d "${TMPDIR:-/tmp}/verba-notices.XXXXXX")"
cleanup() {
    /bin/rm -rf "${work_dir}"
}
trap cleanup EXIT

metadata_path="${work_dir}/metadata.json"
notice_path="${work_dir}/THIRD_PARTY_NOTICES.md"

cd "${repo_root}"
cargo metadata --format-version 1 --locked > "${metadata_path}"
lock_sha256="$(/usr/bin/shasum -a 256 Cargo.lock | /usr/bin/awk '{ print $1 }')"
package_count="$(jq '[.packages[] | select(.source != null)] | length' "${metadata_path}")"

{
    echo "# Third-party notices"
    echo
    echo "Verba is built with the Rust packages listed below. This is a conservative inventory of all external packages in the locked workspace dependency graph, including build and development dependencies."
    echo
    echo "Generated from \`Cargo.lock\` SHA-256 \`${lock_sha256}\`. Package count: ${package_count}. Regenerate with \`./scripts/generate-third-party-notices.sh\`."
    echo
    echo "Each package remains copyright its respective authors and is provided under the SPDX license expression shown. The linked crates.io source distribution contains the authoritative license and notice files for that version. Verba's inclusion of a package does not change its license terms."
    echo
    echo "| Package | Version | License | Source |"
    echo "| --- | --- | --- | --- |"
    jq -r '
        [.packages[] | select(.source != null)]
        | sort_by(.name, .version)
        | .[]
        | [
            .name,
            .version,
            (.license // "Not specified in package metadata"),
            ("https://crates.io/crates/" + .name + "/" + .version)
          ]
        | @tsv
    ' "${metadata_path}" | while IFS=$'\t' read -r name version license source; do
        printf '| `%s` | `%s` | `%s` | [crates.io](%s) |\n' \
            "${name}" "${version}" "${license}" "${source}"
    done
} > "${notice_path}"

/bin/mkdir -p "$(dirname "${output_path}")"
/bin/cp "${notice_path}" "${output_path}"

echo "Wrote ${package_count} package notices to ${output_path}"
