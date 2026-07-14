# Verba 1.0.0 release test matrix

This is the manual sign-off record for the exact notarized Verba 1.0.0 release candidate. Automated tests do not replace these system, application, permission, display, Accessibility, and clean-account checks.

Use `Pass`, `Fail`, or `Blocked` for every result. A release-blocking row may not be `Fail` or `Blocked` when the project owner signs off.

## Candidate record

| Field | Value |
| --- | --- |
| Artifact | `Verba-1.0.0-4-arm64-notarized.zip` |
| SHA-256 | Pending |
| Source revision | Pending |
| Source state in manifest | Must be `clean` |
| Notarization submission ID | Pending |
| Tester | Pending |
| Test date | Pending |
| macOS version | Pending |
| Mac model / CPU / memory | Pending |
| Clean user account | Pending |
| Chromium or Electron app / version | Pending |
| Display arrangement | Pending |
| OpenAI API project | Personal test project; never record the key |

## Rejected candidate history

| Build | SHA-256 | Notarization | Rejection reason |
| --- | --- | --- | --- |
| 1.0.0 (2) | `2fc531cd8e4ddefb746db0c2582b447b4ef7a1dea586a3114b2e6caef53c82be` | Accepted, submission `08560573-446e-4ede-8988-94d08f23ce6a` | Rejected during manual permission testing: the menu did not refresh after macOS granted Accessibility access, leaving both actions disabled until another lifecycle refresh. |
| 1.0.0 (3) | `37a5492d00494b7612b28c06f5df7f79c6e9625e091bc6fa2ddf8d8e58e60202` | Accepted, submission `80809719-6038-4385-a878-289f8550c5e2` | Rejected during manual translation testing: preparing missing language assets with automatic source detection failed before macOS received source text. |

## Automated candidate verification

| Check | Result | Evidence |
| --- | --- | --- |
| Rust formatting, Clippy, Rust tests, and macOS host tests | Pass | `./scripts/check.sh` passed for the build 4 source tree, including explicit-source translation preparation and the selected German text regression. |
| RustSec, dependency licenses, sources, and notices | Pass | `./scripts/security-check.sh` passed; the accepted duplicate `winnow` versions remain documented in `SECURITY.md`. |
| Developer ID signing | Pending | |
| Apple notarization | Pending | |
| Stapling and Gatekeeper | Pending | |
| Final artifact checksum | Pending | |

## Artifact and clean installation

Run these rows using only the candidate ZIP and checksum copied to an Apple-silicon Mac user account that has never run bundle identifier `io.github.the-pavels.verba`.

| ID | Procedure | Expected result | Result | Observation |
| --- | --- | --- | --- | --- |
| ART-01 | Run `shasum -a 256 -c Verba-1.0.0-4-arm64-notarized.zip.sha256`. | The exact candidate reports `OK`. | Not run | |
| ART-02 | Extract the ZIP, move Verba to `/Applications`, and open it from Finder without a Gatekeeper bypass. | Verba opens normally; Gatekeeper shows no malware or unidentified-developer failure. | Not run | |
| ART-03 | Inspect About and support diagnostics. | App and Rust core are 1.0.0, build is 4, architecture is arm64, and diagnostics contain no content or credential. | Not run | |
| ART-04 | Inspect the menu bar and Dock. | Verba appears in the menu bar and has no persistent Dock icon. | Not run | |
| ART-05 | Quit and reopen Verba. | The app exits cleanly and starts normally without duplicate menu items or shortcut registrations. | Not run | |

## Supported applications

For every row, copy an unrelated rich clipboard fixture first. Run Translate and Proofread, verify the selected text is reproduced accurately, verify no source text is replaced, then paste the original fixture back into its source application and confirm its representations survived.

| ID | Application and selection | Translate | Proofread | Clipboard | Observation |
| --- | --- | --- | --- | --- | --- |
| APP-01 | TextEdit plain text: one sentence | Not run | Not run | Not run | |
| APP-02 | TextEdit rich text: styled text across two lines | Not run | Not run | Not run | |
| APP-03 | Notes: paragraphs plus a list item | Not run | Not run | Not run | |
| APP-04 | Safari: heading, punctuation, and non-ASCII body text | Not run | Not run | Not run | |
| APP-05 | Mail: text in a received message | Not run | Not run | Not run | |
| APP-06 | Mail compose: editable text across two lines | Not run | Not run | Not run | |
| APP-07 | Recorded Chromium/Electron app: editable text | Not run | Not run | Not run | |
| APP-08 | Recorded Chromium/Electron app: rendered text | Not run | Not run | Not run | |

## Clipboard and capture failures

| ID | Procedure | Expected result | Result | Observation |
| --- | --- | --- | --- | --- |
| CAP-01 | Capture with an initially empty clipboard. | Capture succeeds and the clipboard is empty afterward. | Not run | |
| CAP-02 | Repeat with unrelated plain text, styled text, an image, and two Finder files. | Every original item and representation pastes unchanged after capture. | Not run | |
| CAP-03 | Copy a new value immediately after invoking an action. | Verba cancels restoration and the newer clipboard value survives. | Not run | |
| CAP-04 | Invoke an action without a selection. | A selection timeout is actionable and the prior clipboard survives. | Not run | |
| CAP-05 | Select only an image or unsupported content. | Verba requests a text selection and restores the prior clipboard. | Not run | |
| CAP-06 | Select a disposable value in a secure field. | Verba rejects secure capture; the value never appears in the clipboard or popup. | Not run | Never use a real secret. |
| CAP-07 | Quit the source application immediately after invocation. | Verba remains responsive, reports a safe failure, and preserves the safest current clipboard value. | Not run | |
| CAP-08 | Test 9,999, 10,000, and 10,001-character selections for both actions. | The first two are accepted; 10,001 is rejected before translation or network processing. | Not run | |

## Accessibility permission

| ID | Procedure | Expected result | Result | Observation |
| --- | --- | --- | --- | --- |
| PERM-01 | Start with Verba absent from Accessibility and choose Request Accessibility Access. | Verba explains the reason before explicitly opening the macOS permission flow. | Not run | |
| PERM-02 | Deny or leave access disabled and retry capture. | Status and error explain that access is required; the system prompt is not shown repeatedly. | Not run | |
| PERM-03 | Choose Open Accessibility Settings. | System Settings opens to Privacy & Security > Accessibility. | Not run | |
| PERM-04 | Grant access while Verba is running, return, and invoke both shortcuts. | Status refreshes and both captures work, or a required relaunch is clearly recoverable. | Not run | |
| PERM-05 | Revoke access while Verba is running and retry. | Capture is denied without changing the clipboard. | Not run | |
| PERM-06 | Grant access again and retry. | The new action succeeds and no stale error replaces its result. | Not run | |

## Translation

| ID | Procedure | Expected result | Result | Observation |
| --- | --- | --- | --- | --- |
| TR-01 | Translate a supported language pair whose assets are already installed. | The correct target language and detected source are shown. | Not run | Record languages. |
| TR-02 | Select a supported target whose language assets are absent. | macOS prepares/downloads resources and the action recovers without losing the request. | Not run | Record languages and network state. |
| TR-03 | Disable network after required assets are installed and translate again. | Translation succeeds offline. | Not run | |
| TR-04 | Translate text already in the target language. | The same-language outcome is clear and non-destructive. | Not run | |
| TR-05 | Attempt an unsupported pair. | Verba shows an actionable target-language error without stale output. | Not run | |
| TR-06 | Change the target language in Settings and invoke Translate without relaunching. | The next action uses the new persisted target. | Not run | |

## Proofreading and Keychain

| ID | Procedure | Expected result | Result | Observation |
| --- | --- | --- | --- | --- |
| PROOF-01 | Invoke Proofread before accepting its disclosure. | Selected text is not sent until the disclosure is accepted. Cancel leaves no request or acknowledgement. | Not run | |
| PROOF-02 | Save a valid API key and test the connection. | The connection succeeds; the UI and diagnostics expose only configured state. | Not run | |
| PROOF-03 | Proofread text with errors and text with no issues. | Structured corrected and no-issues outcomes render distinctly and can be copied. | Not run | |
| PROOF-04 | Quit and reopen Verba, then proofread again. | Disclosure, settings, and Keychain key persist without exposing the key. | Not run | |
| PROOF-05 | Test an invalid key and an account without available quota. | Authentication and quota failures have distinct actionable messages and no raw provider body. | Not run | |
| PROOF-06 | Disable network and invoke Proofread. | The action times out or reports offline service failure within the documented bound; Verba remains responsive. | Not run | |
| PROOF-07 | Delete the API key in Settings and invoke Proofread. | The Keychain item is removed and Verba directs the user to configure a key. | Not run | |
| PROOF-08 | Inspect the OpenAI project usage after a known request. | Only explicitly invoked proofreading requests are attributable to the test; no background requests occur. | Not run | Do not record the key. |

## Shortcuts, cancellation, and popup

| ID | Procedure | Expected result | Result | Observation |
| --- | --- | --- | --- | --- |
| UI-01 | Record new valid shortcuts, quit, and reopen. | Both registrations update immediately and persist. | Not run | |
| UI-02 | Try a collision, reserved combination, and shortcut without a primary modifier. | Each is rejected and the previous working shortcut remains registered. | Not run | |
| UI-03 | Press the same shortcut repeatedly while its action loads. | Only one matching request remains active. | Not run | |
| UI-04 | Alternate both shortcuts ten times using identifiable selections. | Only the latest action completes visibly; the app and clipboard remain stable. | Not run | |
| UI-05 | Dismiss loading with Escape and by clicking outside. | The popup stays dismissed and late work does not make it reappear. | Not run | |
| UI-06 | Invoke near every corner of each available display and after rearranging displays. | The popup remains inside the correct visible frame and follows the pointer's display. | Not run | |
| UI-07 | Disconnect a secondary display while a popup is visible. | Verba remains responsive and the next popup is fully visible on a remaining display. | Not run | |

## Accessibility and visual behavior

| ID | Procedure | Expected result | Result | Observation |
| --- | --- | --- | --- | --- |
| A11Y-01 | Use menu, Settings, disclosure, popup, result copy, and recovery actions with keyboard only. | Every control is reachable, has a visible focus state, and activates correctly. | Not run | |
| A11Y-02 | Repeat the primary flows with VoiceOver. | Labels, values, headings, order, status changes, and recovery actions are understandable. | Not run | |
| A11Y-03 | Use larger macOS text/accessibility display settings. | Text remains readable, essential content is not clipped, and long content scrolls. | Not run | |
| A11Y-04 | Enable Reduce Motion and repeat popup presentation/dismissal. | Motion is removed or reduced without losing state feedback. | Not run | |
| A11Y-05 | Check loading, success, no-issues, error, disabled, focus, and link colors in light and dark appearance. | Information is not conveyed by color alone and remains legible. | Not run | |
| A11Y-06 | Return focus after dismissing a popup using keyboard and pointer paths. | Focus restoration is predictable and does not activate or edit the source unexpectedly. | Not run | |

## Uninstall, reinstall, and rollback

| ID | Procedure | Expected result | Result | Observation |
| --- | --- | --- | --- | --- |
| LIFE-01 | Remove only `Verba.app`, then reinstall the same candidate. | Preferences, disclosure, and Keychain key survive normal reinstall. | Not run | |
| LIFE-02 | Perform every complete-cleanup step in `PRIVACY.md`, then reinstall. | Defaults, key, and Accessibility grant are absent; Verba behaves as a first launch. | Not run | |
| LIFE-03 | Replace 1.0.0 with the last qualified pre-release build, then restore 1.0.0. | Both notarized apps open normally; compatible preferences and Keychain scope survive replacement. | Not run | |

## Performance and final inspection

| ID | Procedure | Expected result | Result | Observation |
| --- | --- | --- | --- | --- |
| FINAL-01 | Measure 20 cold launches and 20 invocations per action on the oldest supported Mac using the documented signposts. | Every p95 budget in `README.md` passes. | Not run | Attach the Instruments summary; never attach selected text. |
| FINAL-02 | Download the published candidate and checksum into a new directory and verify again. | Its SHA-256 matches this record and the tested local candidate. | Not run | |
| FINAL-03 | Inspect the extracted bundle and public release assets. | No source PDF, API key, signing/notary credential, dSYM, archive, source path, or unintended generated file is present. | Not run | |
| FINAL-04 | Review `CHANGELOG.md`, `PRIVACY.md`, `SECURITY.md`, third-party notices, installation instructions, limitations, and rollback guidance. | Published documentation matches the candidate behavior. | Not run | |

## Project-owner sign-off

I confirm that every release-blocking row above passed against the exact artifact and SHA-256 recorded at the top, all automated checks passed on its clean source revision, and any non-blocking limitation is disclosed in the release notes.

| Field | Value |
| --- | --- |
| Project owner | Pending |
| Decision | Pending |
| Date | Pending |
| Notes / linked evidence | Pending |
