#!/bin/bash

set -euo pipefail

app_path="${1:?usage: verify-release.sh APP_PATH VERSION BUILD_NUMBER [SIGNING_MODE] [TEAM_ID]}"
expected_version="${2:?usage: verify-release.sh APP_PATH VERSION BUILD_NUMBER [SIGNING_MODE] [TEAM_ID]}"
expected_build="${3:?usage: verify-release.sh APP_PATH VERSION BUILD_NUMBER [SIGNING_MODE] [TEAM_ID]}"
signing_mode="${4:-unsigned}"
expected_team_id="${5:-}"
expected_arch="arm64"
expected_bundle_id="io.github.the-pavels.verba"
expected_feed_url="https://github.com/the-pavels/verba/releases/latest/download/appcast.xml"
expected_update_key="l6C6I+bPA3bNxsnJHRJs8nN7ci53kw5VVH7MkzvOyPU="
expected_sparkle_version="2.9.2"
info_plist="${app_path}/Contents/Info.plist"
executable="${app_path}/Contents/MacOS/Verba"
resources="${app_path}/Contents/Resources"
privacy_manifest="${resources}/PrivacyInfo.xcprivacy"
localized_strings="${resources}/en.lproj/Localizable.strings"
sparkle_framework="${app_path}/Contents/Frameworks/Sparkle.framework"
sparkle_info="${sparkle_framework}/Versions/B/Resources/Info.plist"

fail() {
    echo "Release verification failed: $*" >&2
    exit 1
}

plist_value() {
    /usr/libexec/PlistBuddy -c "Print :$2" "$1" 2>/dev/null
}

case "${signing_mode}" in
    unsigned)
        [[ -z "${expected_team_id}" ]] || fail "unsigned verification must not specify a team ID"
        ;;
    developer-id | notarized)
        [[ "${expected_team_id}" =~ ^[A-Z0-9]{10}$ ]] || fail "Developer ID verification requires a valid team ID"
        ;;
    *)
        fail "unsupported signing mode ${signing_mode}"
        ;;
esac

[[ -d "${app_path}" ]] || fail "missing app bundle at ${app_path}"
[[ -f "${info_plist}" ]] || fail "missing Contents/Info.plist"
[[ -x "${executable}" ]] || fail "missing executable Contents/MacOS/Verba"

[[ "$(plist_value "${info_plist}" CFBundleExecutable)" == "Verba" ]] || fail "unexpected executable name"
[[ "$(plist_value "${info_plist}" CFBundlePackageType)" == "APPL" ]] || fail "unexpected package type"
[[ "$(plist_value "${info_plist}" CFBundleShortVersionString)" == "${expected_version}" ]] || fail "marketing version does not match ${expected_version}"
[[ "$(plist_value "${info_plist}" CFBundleVersion)" == "${expected_build}" ]] || fail "build number does not match ${expected_build}"
[[ "$(plist_value "${info_plist}" LSMinimumSystemVersion)" == "15.0" ]] || fail "deployment target is not macOS 15.0"
[[ "$(plist_value "${info_plist}" LSUIElement)" == "true" ]] || fail "app is not configured as a menu-bar-only UI element"
[[ "$(plist_value "${info_plist}" SUFeedURL)" == "${expected_feed_url}" ]] || fail "unexpected Sparkle feed URL"
[[ "$(plist_value "${info_plist}" SUPublicEDKey)" == "${expected_update_key}" ]] || fail "unexpected Sparkle public update key"
[[ "$(plist_value "${info_plist}" SURequireSignedFeed)" == "true" ]] || fail "signed Sparkle feeds are not required"
[[ "$(plist_value "${info_plist}" SUVerifyUpdateBeforeExtraction)" == "true" ]] || fail "updates are not verified before extraction"
[[ "$(plist_value "${info_plist}" SUEnableAutomaticChecks)" == "false" ]] || fail "automatic update checks must be opt-in"
[[ "$(plist_value "${info_plist}" SUAutomaticallyUpdate)" == "false" ]] || fail "silent automatic update installation must be disabled"

bundle_id="$(plist_value "${info_plist}" CFBundleIdentifier)"
[[ "${bundle_id}" == "${expected_bundle_id}" ]] || fail "bundle identifier is not ${expected_bundle_id}"

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
[[ "${localized_copy}" == *"Update checks contact GitHub without sending selected text or your API key."* ]] || fail "update-check privacy disclosure is missing"

archs="$(/usr/bin/lipo -archs "${executable}")"
[[ "${archs}" == "${expected_arch}" ]] || fail "expected ${expected_arch} executable, found ${archs}"

for unexpected_dir in PlugIns XPCServices Helpers; do
    [[ ! -e "${app_path}/Contents/${unexpected_dir}" ]] || fail "unexpected embedded code directory Contents/${unexpected_dir}"
done

[[ -d "${sparkle_framework}" ]] || fail "Sparkle.framework is missing"
[[ -f "${sparkle_info}" ]] || fail "Sparkle framework Info.plist is missing"
[[ "$(plist_value "${sparkle_info}" CFBundleIdentifier)" == "org.sparkle-project.Sparkle" ]] || fail "unexpected Sparkle framework bundle identifier"
[[ "$(plist_value "${sparkle_info}" CFBundleShortVersionString)" == "${expected_sparkle_version}" ]] || fail "expected Sparkle ${expected_sparkle_version}"

framework_entry_count="$(/usr/bin/find "${app_path}/Contents/Frameworks" -mindepth 1 -maxdepth 1 -print | /usr/bin/wc -l | /usr/bin/tr -d ' ')"
[[ "${framework_entry_count}" == "1" ]] || fail "Contents/Frameworks must contain only Sparkle.framework"

sparkle_symlink_count=0
while IFS= read -r bundle_link; do
    relative_path="${bundle_link#"${app_path}/"}"
    link_target="$(/usr/bin/readlink "${bundle_link}")"
    case "${relative_path}:${link_target}" in
        Contents/Frameworks/Sparkle.framework/Autoupdate:Versions/Current/Autoupdate | \
        Contents/Frameworks/Sparkle.framework/Resources:Versions/Current/Resources | \
        Contents/Frameworks/Sparkle.framework/Sparkle:Versions/Current/Sparkle | \
        Contents/Frameworks/Sparkle.framework/Updater.app:Versions/Current/Updater.app | \
        Contents/Frameworks/Sparkle.framework/Versions/Current:B | \
        Contents/Frameworks/Sparkle.framework/XPCServices:Versions/Current/XPCServices)
            ;;
        *)
            fail "unexpected bundle symlink ${relative_path} -> ${link_target}"
            ;;
    esac
    ((sparkle_symlink_count += 1))
done < <(/usr/bin/find "${app_path}" -type l -print | LC_ALL=C /usr/bin/sort)
[[ "${sparkle_symlink_count}" -eq 6 ]] || fail "expected six Sparkle framework symlinks, found ${sparkle_symlink_count}"

mach_o_count=0
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
        Contents/Frameworks/Sparkle.framework/*)
            ;;
        Contents/_CodeSignature/CodeResources)
            [[ "${signing_mode}" != "unsigned" ]] || fail "unsigned bundle contains code-signing resources"
            ;;
        Contents/CodeResources)
            [[ "${signing_mode}" == "notarized" ]] || fail "unstapled bundle contains notarization ticket resources"
            ;;
        *)
            fail "unexpected bundle file ${relative_path}"
            ;;
    esac

    if /usr/bin/file -b "${bundle_file}" | /usr/bin/grep -q 'Mach-O'; then
        ((mach_o_count += 1))
        case "${relative_path}" in
            Contents/MacOS/Verba)
                [[ "$(/usr/bin/lipo -archs "${bundle_file}")" == "${expected_arch}" ]] || fail "expected ${expected_arch} executable at ${relative_path}"
                ;;
            Contents/Frameworks/Sparkle.framework/Versions/B/Autoupdate | \
            Contents/Frameworks/Sparkle.framework/Versions/B/Sparkle | \
            Contents/Frameworks/Sparkle.framework/Versions/B/Updater.app/Contents/MacOS/Updater | \
            Contents/Frameworks/Sparkle.framework/Versions/B/XPCServices/Downloader.xpc/Contents/MacOS/Downloader | \
            Contents/Frameworks/Sparkle.framework/Versions/B/XPCServices/Installer.xpc/Contents/MacOS/Installer)
                sparkle_archs="$(/usr/bin/lipo -archs "${bundle_file}")"
                [[ "${sparkle_archs}" == "x86_64 arm64" || "${sparkle_archs}" == "arm64 x86_64" ]] || fail "unexpected Sparkle architectures at ${relative_path}: ${sparkle_archs}"
                ;;
            *)
                fail "unexpected embedded Mach-O code at ${relative_path}"
                ;;
        esac
    fi
done < <(/usr/bin/find "${app_path}" -type f -print | LC_ALL=C /usr/bin/sort)

[[ "${mach_o_count}" -eq 6 ]] || fail "expected six embedded Mach-O files, found ${mach_o_count}"

if /usr/bin/otool -L "${executable}" | /usr/bin/grep -E '/Users/|/target/' >/dev/null; then
    fail "executable contains a non-system development library path"
fi
if ! /usr/bin/otool -L "${executable}" | /usr/bin/grep -F '@rpath/Sparkle.framework/Versions/B/Sparkle' >/dev/null; then
    fail "executable is not linked to the embedded Sparkle framework"
fi

if [[ "${signing_mode}" == "unsigned" ]]; then
    [[ ! -e "${app_path}/Contents/_CodeSignature" ]] || fail "unsigned package contains a bundle signature"
else
    [[ -f "${app_path}/Contents/_CodeSignature/CodeResources" ]] || fail "Developer ID bundle signature resources are missing"
    if [[ "${signing_mode}" == "notarized" ]]; then
        [[ -f "${app_path}/Contents/CodeResources" ]] || fail "stapled notarization ticket resources are missing"
    else
        [[ ! -e "${app_path}/Contents/CodeResources" ]] || fail "unstapled bundle contains notarization ticket resources"
    fi
    /usr/bin/codesign --verify --deep --strict=all --verbose=2 "${app_path}" || fail "Developer ID signature verification failed"

    signing_info="$(/usr/bin/codesign -dvvv "${app_path}" 2>&1)"
    [[ "${signing_info}" == *"Identifier=${expected_bundle_id}"* ]] || fail "signed identifier does not match ${expected_bundle_id}"
    [[ "${signing_info}" == *"Authority=Developer ID Application:"* ]] || fail "signature does not use a Developer ID Application certificate"
    [[ "${signing_info}" == *"TeamIdentifier=${expected_team_id}"* ]] || fail "signature team does not match ${expected_team_id}"
    [[ "${signing_info}" == *"Timestamp="* ]] || fail "signature has no secure timestamp"
    /usr/bin/grep -Eq 'flags=.*\([^)]*runtime[^)]*\)' <<< "${signing_info}" || fail "hardened runtime flag is missing"

    entitlements="$(/usr/bin/codesign -d --entitlements - --xml "${app_path}" 2>/dev/null)"
    if /usr/bin/grep -q '<key>' <<< "${entitlements}"; then
        fail "release signature contains an unexpected entitlement"
    fi
fi

echo "Verified ${app_path}"
echo "  bundle: ${bundle_id}"
echo "  version: ${expected_version} (${expected_build})"
echo "  architecture: ${expected_arch}"
echo "  signing: ${signing_mode}"
echo "  updates: Sparkle ${expected_sparkle_version}, signed feed, opt-in checks"
echo "  privacy: manifest and localized permission disclosures verified"
