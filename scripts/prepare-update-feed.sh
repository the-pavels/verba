#!/bin/bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
archive_path="${1:?usage: prepare-update-feed.sh NOTARIZED_ZIP RELEASE_TAG [OUTPUT_PATH]}"
release_tag="${2:?usage: prepare-update-feed.sh NOTARIZED_ZIP RELEASE_TAG [OUTPUT_PATH]}"
output_path="${3:-${repo_root}/dist/appcast.xml}"
team_id="${VERBA_DEVELOPMENT_TEAM:-}"
key_account="${VERBA_UPDATE_KEY_ACCOUNT:-io.github.the-pavels.verba}"
derived_data_path="${VERBA_DERIVED_DATA_PATH:-${TMPDIR:-/tmp}/verba-update-feed-derived-data}"
generate_appcast="${SPARKLE_GENERATE_APPCAST:-}"
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/verba-update-feed.XXXXXX")"

cleanup() {
    exit_status=$?
    trap - EXIT
    /bin/rm -rf "${work_dir}"
    exit "${exit_status}"
}

fail() {
    echo "Update feed preparation failed: $*" >&2
    exit 1
}

plist_value() {
    /usr/libexec/PlistBuddy -c "Print :$2" "$1" 2>/dev/null
}

trap cleanup EXIT

[[ -f "${archive_path}" ]] || fail "missing notarized ZIP at ${archive_path}"
[[ "${release_tag}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "release tag must use vMAJOR.MINOR.PATCH format"
[[ "${team_id}" =~ ^[A-Z0-9]{10}$ ]] || fail "VERBA_DEVELOPMENT_TEAM must be a 10-character Apple team ID"
[[ -n "${key_account}" && "${key_account}" != *$'\n'* ]] || fail "VERBA_UPDATE_KEY_ACCOUNT must be a nonempty single-line account name"

extracted_dir="${work_dir}/extracted"
/bin/mkdir -p "${extracted_dir}"
/usr/bin/ditto -x -k "${archive_path}" "${extracted_dir}"
app_path="${extracted_dir}/Verba.app"
info_plist="${app_path}/Contents/Info.plist"
[[ -f "${info_plist}" ]] || fail "ZIP must contain Verba.app at its root"

version="$(plist_value "${info_plist}" CFBundleShortVersionString || true)"
build_number="$(plist_value "${info_plist}" CFBundleVersion || true)"
[[ "${release_tag}" == "v${version}" ]] || fail "release tag ${release_tag} does not match app version ${version}"
[[ "${build_number}" =~ ^[1-9][0-9]*$ ]] || fail "app has an invalid build number"

"${repo_root}/scripts/verify-release.sh" \
    "${app_path}" \
    "${version}" \
    "${build_number}" \
    notarized \
    "${team_id}"
/usr/bin/codesign --test-requirement="=notarized" --verify --verbose=2 "${app_path}" || fail "app does not satisfy the notarization requirement"
/usr/bin/xcrun stapler validate "${app_path}" || fail "app has no valid stapled notarization ticket"

if [[ -z "${generate_appcast}" ]]; then
    xcodebuild \
        -resolvePackageDependencies \
        -project "${repo_root}/macos/Verba.xcodeproj" \
        -scheme Verba \
        -derivedDataPath "${derived_data_path}"
    generate_appcast="${derived_data_path}/SourcePackages/artifacts/sparkle/Sparkle/bin/generate_appcast"
fi
[[ -x "${generate_appcast}" ]] || fail "Sparkle generate_appcast is unavailable; set SPARKLE_GENERATE_APPCAST to its executable path"

archives_dir="${work_dir}/archives"
/bin/mkdir -p "${archives_dir}"
archive_name="$(basename "${archive_path}")"
/bin/cp "${archive_path}" "${archives_dir}/${archive_name}"
archive_digest="$(/usr/bin/shasum -a 256 "${archives_dir}/${archive_name}" | /usr/bin/awk '{ print $1 }')"
download_prefix="https://github.com/the-pavels/verba/releases/download/${release_tag}/"

"${generate_appcast}" \
    --account "${key_account}" \
    --download-url-prefix "${download_prefix}" \
    --link "https://github.com/the-pavels/verba/releases/tag/${release_tag}" \
    --maximum-versions 1 \
    --maximum-deltas 0 \
    "${archives_dir}"

generated_appcast="${archives_dir}/appcast.xml"
[[ -f "${generated_appcast}" ]] || fail "Sparkle did not generate appcast.xml"
/usr/bin/xmllint --noout "${generated_appcast}" || fail "generated appcast is not valid XML"
/usr/bin/grep -Fq "sparkle:edSignature=" "${generated_appcast}" || fail "generated appcast has no Ed25519 archive signature"
/usr/bin/grep -Fq "<sparkle:version>${build_number}</sparkle:version>" "${generated_appcast}" || fail "generated appcast has the wrong build number"
/usr/bin/grep -Fq "sparkle-signatures:" "${generated_appcast}" || fail "generated appcast has no signed-feed footer"
/usr/bin/grep -Fq "edSignature:" "${generated_appcast}" || fail "generated appcast has no signed-feed signature"
/usr/bin/grep -Fq "${download_prefix}${archive_name}" "${generated_appcast}" || fail "generated appcast has the wrong archive URL"

generated_archive_digest="$(/usr/bin/shasum -a 256 "${archives_dir}/${archive_name}" | /usr/bin/awk '{ print $1 }')"
[[ "${generated_archive_digest}" == "${archive_digest}" ]] || fail "Sparkle modified the notarized archive"

/bin/mkdir -p "$(dirname "${output_path}")"
/usr/bin/install -m 0644 "${generated_appcast}" "${output_path}"

echo "Created ${output_path}"
echo "Upload $(basename "${output_path}") and ${archive_name} to GitHub release ${release_tag}"
echo "Installed apps read the feed from https://github.com/the-pavels/verba/releases/latest/download/appcast.xml"
