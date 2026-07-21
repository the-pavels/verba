#!/bin/bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
workspace_version="$(/usr/bin/awk '
    /^\[workspace.package\]$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^version = / { gsub(/[\" ]/, "", $3); print $3; exit }
' "${repo_root}/Cargo.toml")"
version="${1:-${workspace_version}}"
build_number="${2:-14}"
release_arch="arm64"
team_id="${VERBA_DEVELOPMENT_TEAM:-}"
notary_profile="${VERBA_NOTARY_KEYCHAIN_PROFILE:-verba-notary}"
notary_timeout="${VERBA_NOTARY_TIMEOUT:-30m}"
resume_submission_id="${VERBA_NOTARY_SUBMISSION_ID:-}"
dist_dir="${VERBA_DIST_DIR:-${repo_root}/dist}"
work_dir="$(mktemp -d "${TMPDIR:-/tmp}/verba-notarize.XXXXXX")"

cleanup() {
    exit_status=$?
    trap - EXIT
    /bin/rm -rf "${work_dir}"
    exit "${exit_status}"
}

fail() {
    echo "Notarization failed: $*" >&2
    exit 1
}

plist_string() {
    /usr/bin/plutil -extract "$2" raw -expect string -o - "$1" 2>/dev/null
}

verify_notarized_app() {
    app_path="$1"

    "${repo_root}/scripts/verify-release.sh" \
        "${app_path}" \
        "${version}" \
        "${build_number}" \
        notarized \
        "${team_id}"
    /usr/bin/codesign --test-requirement="=notarized" --verify --verbose=2 "${app_path}"
    /usr/bin/xcrun stapler validate "${app_path}"

    assessment="$(/usr/sbin/spctl --assess --type execute --verbose=4 "${app_path}" 2>&1)" || {
        echo "${assessment}" >&2
        fail "Gatekeeper rejected ${app_path}"
    }
    echo "${assessment}"
    /usr/bin/grep -Fq "source=Notarized Developer ID" <<< "${assessment}" || \
        fail "Gatekeeper assessment did not identify a notarized Developer ID app"
}

trap cleanup EXIT

if [[ ! "${version}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    fail "version must use MAJOR.MINOR.PATCH format: ${version}"
fi
if [[ ! "${build_number}" =~ ^[1-9][0-9]*$ ]]; then
    fail "build number must be a positive integer: ${build_number}"
fi
if [[ "${version}" != "${workspace_version}" ]]; then
    fail "release version ${version} does not match Cargo workspace version ${workspace_version}"
fi
if [[ ! "${team_id}" =~ ^[A-Z0-9]{10}$ ]]; then
    fail "VERBA_DEVELOPMENT_TEAM must be a 10-character Apple team ID"
fi
if [[ -z "${VERBA_SIGNING_IDENTITY:-}" ]]; then
    fail "VERBA_SIGNING_IDENTITY is required for Developer ID signing"
fi
if [[ -z "${notary_profile}" || "${notary_profile}" == *$'\n'* ]]; then
    fail "VERBA_NOTARY_KEYCHAIN_PROFILE must be a nonempty single-line profile name"
fi
if [[ ! "${notary_timeout}" =~ ^[1-9][0-9]*[smh]?$ ]]; then
    fail "VERBA_NOTARY_TIMEOUT must be an integer optionally followed by s, m, or h"
fi
if [[ -n "${resume_submission_id}" && ! "${resume_submission_id}" =~ ^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$ ]]; then
    fail "VERBA_NOTARY_SUBMISSION_ID must be a UUID"
fi

mkdir -p "${dist_dir}"
cd "${repo_root}"

echo "Validating notarytool Keychain profile ${notary_profile}"
if ! /usr/bin/xcrun notarytool history \
    --keychain-profile "${notary_profile}" \
    --output-format json \
    >/dev/null; then
    fail "Keychain profile ${notary_profile} is missing or invalid"
fi

signed_basename="Verba-${version}-${build_number}-${release_arch}-developer-id"
signed_artifact="${dist_dir}/${signed_basename}.zip"
signed_manifest="${dist_dir}/${signed_basename}.manifest.txt"
signed_checksum="${signed_artifact}.sha256"

if [[ -z "${resume_submission_id}" ]]; then
    VERBA_SIGNING_MODE=developer-id \
        "${repo_root}/scripts/package-release.sh" "${version}" "${build_number}"
else
    echo "Resuming notarization submission ${resume_submission_id} with the existing signed artifact"
fi

[[ -f "${signed_artifact}" ]] || fail "signed artifact was not created"
[[ -f "${signed_manifest}" ]] || fail "signed manifest was not created"
[[ -f "${signed_checksum}" ]] || fail "signed checksum was not created"
(
    cd "${dist_dir}"
    /usr/bin/shasum -a 256 -c "$(basename "${signed_checksum}")"
)

submission_result="${work_dir}/notary-submission.json"
set +e
if [[ -z "${resume_submission_id}" ]]; then
    /usr/bin/xcrun notarytool submit "${signed_artifact}" \
        --keychain-profile "${notary_profile}" \
        --wait \
        --timeout "${notary_timeout}" \
        --output-format json \
        > "${submission_result}"
else
    /usr/bin/xcrun notarytool info "${resume_submission_id}" \
        --keychain-profile "${notary_profile}" \
        --output-format json \
        > "${submission_result}"
fi
request_status=$?
set -e

[[ -s "${submission_result}" ]] || fail "notarytool returned no submission result"
validated_result="${work_dir}/notary-submission.validated.json"
/usr/bin/plutil -convert json -o "${validated_result}" "${submission_result}" || fail "notarytool returned invalid JSON"
submission_id="$(plist_string "${validated_result}" id || true)"
submission_state="$(plist_string "${validated_result}" status || true)"
[[ -n "${submission_id}" ]] || fail "notarytool result has no submission ID"
if [[ -n "${resume_submission_id}" && "${submission_id}" != "${resume_submission_id}" ]]; then
    fail "notarytool returned a different submission ID"
fi

evidence_basename="${signed_basename}.notary-${submission_id}"
result_path="${dist_dir}/${evidence_basename}.result.json"
log_path="${dist_dir}/${evidence_basename}.log.json"
converted_result="${work_dir}/${evidence_basename}.result.json"
/usr/bin/plutil -convert json -o "${converted_result}" "${validated_result}"
/usr/bin/install -m 0644 "${converted_result}" "${result_path}"

notary_log="${work_dir}/notary-log.json"
if ! /usr/bin/xcrun notarytool log \
    --keychain-profile "${notary_profile}" \
    "${submission_id}" \
    "${notary_log}"; then
    fail "could not retrieve notarization log for ${submission_id}; inspect ${result_path}"
fi

converted_log="${work_dir}/${evidence_basename}.log.json"
/usr/bin/plutil -convert json -o "${converted_log}" "${notary_log}"
/usr/bin/install -m 0644 "${converted_log}" "${log_path}"

[[ "${request_status}" -eq 0 ]] || fail "notarytool request exited with status ${request_status}; inspect ${log_path}"
[[ "${submission_state}" == "Accepted" ]] || fail "submission ${submission_id} finished with status ${submission_state}; inspect ${log_path}"

log_state="$(plist_string "${notary_log}" status || true)"
log_submission_id="$(plist_string "${notary_log}" jobId || true)"
log_artifact_digest="$(plist_string "${notary_log}" sha256 | /usr/bin/tr '[:upper:]' '[:lower:]' || true)"
signed_artifact_digest="$(/usr/bin/shasum -a 256 "${signed_artifact}" | /usr/bin/awk '{print $1}')"
[[ "${log_state}" == "Accepted" ]] || fail "notarization log status is ${log_state}"
[[ "${log_submission_id}" == "${submission_id}" ]] || fail "notarization log submission ID does not match"
[[ "${log_artifact_digest}" == "${signed_artifact_digest}" ]] || fail "notarization log digest does not match the submitted ZIP"

issues_type="$(/usr/bin/plutil -type issues "${notary_log}" 2>/dev/null || true)"
case "${issues_type}" in
    "(any)")
        ;;
    array)
        issue_count="$(/usr/bin/plutil -extract issues raw -expect array -o - "${notary_log}")"
        [[ "${issue_count}" -eq 0 ]] || fail "notarization log contains ${issue_count} issue(s); inspect ${log_path}"
        ;;
    *)
        fail "notarization log has an unexpected issues field"
        ;;
esac

stapled_dir="${work_dir}/stapled"
/bin/mkdir -p "${stapled_dir}"
/usr/bin/ditto -x -k "${signed_artifact}" "${stapled_dir}"
stapled_app="${stapled_dir}/Verba.app"
/usr/bin/xcrun stapler staple "${stapled_app}"
/usr/bin/xattr -cr "${stapled_app}"
verify_notarized_app "${stapled_app}"

notarized_basename="Verba-${version}-${build_number}-${release_arch}-notarized"
temporary_artifact="${work_dir}/${notarized_basename}.zip"
temporary_manifest="${work_dir}/${notarized_basename}.manifest.txt"

{
    echo "artifact=${notarized_basename}.zip"
    echo "version=${version}"
    echo "build=${build_number}"
    echo "architecture=${release_arch}"
    echo "signing=developer-id"
    echo "notarization=accepted"
    echo "notary-submission-id=${submission_id}"
    echo "team-id=${team_id}"
    /usr/bin/grep -E '^(source-(revision|state|date-epoch)|rustc-[^=]+|cargo-[^=]+)=' "${signed_manifest}"
    echo "submitted-artifact-sha256=${signed_artifact_digest}"
    echo "notary-log-sha256=$(/usr/bin/shasum -a 256 "${notary_log}" | /usr/bin/awk '{print $1}')"
    echo "bundle-files-sha256:"
    (
        cd "${stapled_app}"
        /usr/bin/find . -type f -print | LC_ALL=C /usr/bin/sort | while IFS= read -r bundle_file; do
            /usr/bin/shasum -a 256 "${bundle_file}"
        done
    )
} > "${temporary_manifest}"

COPYFILE_DISABLE=1 /usr/bin/ditto \
    -c \
    -k \
    --keepParent \
    --norsrc \
    --noextattr \
    --noqtn \
    --noacl \
    --zlibCompressionLevel 9 \
    "${stapled_app}" \
    "${temporary_artifact}"

archive_entries="${work_dir}/${notarized_basename}.entries.txt"
/usr/bin/zipinfo -1 "${temporary_artifact}" > "${archive_entries}"
if /usr/bin/grep -E '(^|/)\._' "${archive_entries}" >/dev/null; then
    /usr/bin/grep -E '(^|/)\._' "${archive_entries}" >&2
    fail "notarized archive contains AppleDouble metadata that can invalidate the app after extraction"
fi

final_check_dir="${work_dir}/final-check"
/bin/mkdir -p "${final_check_dir}"
/usr/bin/ditto -x -k "${temporary_artifact}" "${final_check_dir}"
verify_notarized_app "${final_check_dir}/Verba.app"

portable_check_dir="${work_dir}/portable-check"
/bin/mkdir -p "${portable_check_dir}"
/usr/bin/unzip -q "${temporary_artifact}" -d "${portable_check_dir}"
verify_notarized_app "${portable_check_dir}/Verba.app"

artifact_path="${dist_dir}/${notarized_basename}.zip"
manifest_path="${dist_dir}/${notarized_basename}.manifest.txt"
checksum_path="${artifact_path}.sha256"

/usr/bin/install -m 0644 "${temporary_artifact}" "${artifact_path}"
/usr/bin/install -m 0644 "${temporary_manifest}" "${manifest_path}"
(
    cd "${dist_dir}"
    /usr/bin/shasum -a 256 "$(basename "${artifact_path}")" > "$(basename "${checksum_path}")"
)

echo "Created ${artifact_path}"
echo "Manifest ${manifest_path}"
echo "Checksum ${checksum_path}"
echo "Notary result ${result_path}"
echo "Notary log ${log_path}"
