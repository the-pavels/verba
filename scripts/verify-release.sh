#!/bin/bash

set -euo pipefail

app_path="${1:?usage: verify-release.sh APP_PATH VERSION BUILD_NUMBER}"
expected_version="${2:?usage: verify-release.sh APP_PATH VERSION BUILD_NUMBER}"
expected_build="${3:?usage: verify-release.sh APP_PATH VERSION BUILD_NUMBER}"
expected_arch="arm64"
info_plist="${app_path}/Contents/Info.plist"
executable="${app_path}/Contents/MacOS/Verba"
resources="${app_path}/Contents/Resources"
privacy_manifest="${resources}/PrivacyInfo.xcprivacy"
localized_strings="${resources}/en.lproj/Localizable.strings"

fail() {
    echo "Release verification failed: $*" >&2
    exit 1
}

plist_value() {
    /usr/libexec/PlistBuddy -c "Print :$2" "$1" 2>/dev/null
}

[[ -d "${app_path}" ]] || fail "missing app bundle at ${app_path}"
[[ -f "${info_plist}" ]] || fail "missing Contents/Info.plist"
[[ -x "${executable}" ]] || fail "missing executable Contents/MacOS/Verba"

[[ "$(plist_value "${info_plist}" CFBundleExecutable)" == "Verba" ]] || fail "unexpected executable name"
[[ "$(plist_value "${info_plist}" CFBundlePackageType)" == "APPL" ]] || fail "unexpected package type"
[[ "$(plist_value "${info_plist}" CFBundleShortVersionString)" == "${expected_version}" ]] || fail "marketing version does not match ${expected_version}"
[[ "$(plist_value "${info_plist}" CFBundleVersion)" == "${expected_build}" ]] || fail "build number does not match ${expected_build}"
[[ "$(plist_value "${info_plist}" LSMinimumSystemVersion)" == "15.0" ]] || fail "deployment target is not macOS 15.0"
[[ "$(plist_value "${info_plist}" LSUIElement)" == "true" ]] || fail "app is not configured as a menu-bar-only UI element"

bundle_id="$(plist_value "${info_plist}" CFBundleIdentifier)"
[[ -n "${bundle_id}" ]] || fail "bundle identifier is empty"

icon_file="$(plist_value "${info_plist}" CFBundleIconFile)"
[[ -n "${icon_file}" ]] || fail "CFBundleIconFile is missing"
[[ -f "${resources}/${icon_file}" || -f "${resources}/${icon_file}.icns" ]] || fail "compiled app icon is missing"

[[ -f "${privacy_manifest}" ]] || fail "privacy manifest is missing from Contents/Resources"
/usr/bin/plutil -lint "${privacy_manifest}" >/dev/null || fail "privacy manifest is invalid"
[[ "$(plist_value "${privacy_manifest}" NSPrivacyTracking)" == "false" ]] || fail "privacy tracking must be disabled"
[[ "$(plist_value "${privacy_manifest}" NSPrivacyAccessedAPITypes:0:NSPrivacyAccessedAPIType)" == "NSPrivacyAccessedAPICategoryUserDefaults" ]] || fail "UserDefaults privacy category is missing"
[[ "$(plist_value "${privacy_manifest}" NSPrivacyAccessedAPITypes:0:NSPrivacyAccessedAPITypeReasons:0)" == "CA92.1" ]] || fail "UserDefaults reason CA92.1 is missing"

[[ -f "${localized_strings}" ]] || fail "English localization is missing"
localized_copy="$(/usr/bin/iconv -f UTF-16LE -t UTF-8 "${localized_strings}")"
[[ "${localized_copy}" == *"Proofreading sends the selected text to OpenAI using your API key. Translation remains on this Mac."* ]] || fail "proofreading privacy disclosure is missing"
[[ "${localized_copy}" == *"Required to copy selected text from other applications."* ]] || fail "Accessibility purpose copy is missing"

archs="$(/usr/bin/lipo -archs "${executable}")"
[[ "${archs}" == "${expected_arch}" ]] || fail "expected ${expected_arch} executable, found ${archs}"

for unexpected_dir in Frameworks PlugIns XPCServices Helpers; do
    [[ ! -e "${app_path}/Contents/${unexpected_dir}" ]] || fail "unexpected embedded code directory Contents/${unexpected_dir}"
done

while IFS= read -r bundle_file; do
    relative_path="${bundle_file#"${app_path}/"}"
    case "${relative_path}" in
        Contents/Info.plist | \
        Contents/PkgInfo | \
        Contents/MacOS/Verba | \
        Contents/Resources/AppIcon.icns | \
        Contents/Resources/Assets.car | \
        Contents/Resources/PrivacyInfo.xcprivacy | \
        Contents/Resources/en.lproj/Localizable.strings)
            ;;
        *)
            fail "unexpected bundle file ${relative_path}"
            ;;
    esac

    if [[ "${bundle_file}" != "${executable}" ]] && /usr/bin/file -b "${bundle_file}" | /usr/bin/grep -q 'Mach-O'; then
        fail "unexpected embedded Mach-O code at ${relative_path}"
    fi
done < <(/usr/bin/find "${app_path}" -type f -print | LC_ALL=C /usr/bin/sort)

if /usr/bin/otool -L "${executable}" | /usr/bin/grep -E '/Users/|/target/' >/dev/null; then
    fail "executable contains a non-system development library path"
fi

[[ ! -e "${app_path}/Contents/_CodeSignature" ]] || fail "item 40 package must remain unsigned"

echo "Verified ${app_path}"
echo "  bundle: ${bundle_id}"
echo "  version: ${expected_version} (${expected_build})"
echo "  architecture: ${expected_arch}"
echo "  privacy: manifest and localized permission disclosures verified"
