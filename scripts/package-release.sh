#!/bin/bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
workspace_version="$(/usr/bin/awk '
    /^\[workspace.package\]$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^version = / { gsub(/[\" ]/, "", $3); print $3; exit }
' "${repo_root}/Cargo.toml")"
version="${1:-${workspace_version}}"
build_number="${2:-1}"
release_arch="arm64"
dist_dir="${VERBA_DIST_DIR:-${repo_root}/dist}"
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/verba-release.XXXXXX")"
archive_path="${work_dir}/Verba.xcarchive"
artifact_basename="Verba-${version}-${build_number}-${release_arch}-unsigned"
artifact_path="${dist_dir}/${artifact_basename}.zip"
manifest_path="${dist_dir}/${artifact_basename}.manifest.txt"
checksum_path="${artifact_path}.sha256"

trap '/bin/rm -rf "${work_dir}"' EXIT

if [[ ! "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Version must use MAJOR.MINOR.PATCH format: ${version}" >&2
    exit 1
fi

if [[ ! "${build_number}" =~ ^[1-9][0-9]*$ ]]; then
    echo "Build number must be a positive integer: ${build_number}" >&2
    exit 1
fi

if [[ "${version}" != "${workspace_version}" ]]; then
    echo "Release version ${version} does not match Cargo workspace version ${workspace_version}" >&2
    exit 1
fi

mkdir -p "${dist_dir}"

cd "${repo_root}"

echo "Archiving Verba ${version} (${build_number}) for ${release_arch}"
xcodebuild \
    -quiet \
    -project macos/Verba.xcodeproj \
    -scheme Verba \
    -configuration Release \
    -destination "generic/platform=macOS" \
    -derivedDataPath "${work_dir}/DerivedData" \
    -archivePath "${archive_path}" \
    ARCHS="${release_arch}" \
    ONLY_ACTIVE_ARCH=NO \
    VERBA_RUST_ARCHS="${release_arch}" \
    MARKETING_VERSION="${version}" \
    CURRENT_PROJECT_VERSION="${build_number}" \
    CODE_SIGNING_ALLOWED=NO \
    archive

app_path="${archive_path}/Products/Applications/Verba.app"
"${repo_root}/scripts/verify-release.sh" "${app_path}" "${version}" "${build_number}"

source_revision="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
source_state=clean
if [[ -n "$(git status --porcelain 2>/dev/null)" ]]; then
    source_state=dirty
fi

source_date_epoch="${SOURCE_DATE_EPOCH:-$(git show -s --format=%ct HEAD 2>/dev/null || echo 946684800)}"
if [[ ! "${source_date_epoch}" =~ ^[0-9]+$ ]]; then
    echo "SOURCE_DATE_EPOCH must be an integer: ${source_date_epoch}" >&2
    exit 1
fi
normalized_timestamp="$(/bin/date -u -r "${source_date_epoch}" '+%Y%m%d%H%M.%S')"
/usr/bin/find "${app_path}" -exec /usr/bin/touch -h -t "${normalized_timestamp}" {} +

temporary_manifest="${work_dir}/${artifact_basename}.manifest.txt"
{
    echo "artifact=${artifact_basename}.zip"
    echo "version=${version}"
    echo "build=${build_number}"
    echo "architecture=${release_arch}"
    echo "source-revision=${source_revision}"
    echo "source-state=${source_state}"
    echo "source-date-epoch=${source_date_epoch}"
    echo "bundle-files-sha256:"
    (
        cd "${app_path}"
        /usr/bin/find . -type f -print | LC_ALL=C /usr/bin/sort | while IFS= read -r bundle_file; do
            /usr/bin/shasum -a 256 "${bundle_file}"
        done
    )
} > "${temporary_manifest}"

temporary_artifact="${work_dir}/${artifact_basename}.zip"
COPYFILE_DISABLE=1 /usr/bin/ditto \
    -c \
    -k \
    --keepParent \
    --norsrc \
    --noextattr \
    --noqtn \
    --noacl \
    --zlibCompressionLevel 9 \
    "${app_path}" \
    "${temporary_artifact}"

/usr/bin/install -m 0644 "${temporary_artifact}" "${artifact_path}"
/usr/bin/install -m 0644 "${temporary_manifest}" "${manifest_path}"

(
    cd "${dist_dir}"
    /usr/bin/shasum -a 256 "$(basename "${artifact_path}")" > "$(basename "${checksum_path}")"
)

echo "Created ${artifact_path}"
echo "Manifest ${manifest_path}"
echo "Checksum ${checksum_path}"
