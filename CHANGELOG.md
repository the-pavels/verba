# Changelog

## Unreleased

### Reliability

- Detect ambiguous single-word languages from bounded nearby text when the source app exposes it, without changing the selection or sending the context for translation; use preferred languages as a fallback.

## 1.0.2 - 2026-08-03

### Improved

- Keep the popup fully inside the usable display area on smaller screens, with result content remaining scrollable when its preferred size does not fit.
- Show the target-language dropdown directly when the selected text already uses the target language or the current language pair is unavailable, then retry immediately after another language is chosen.

## 1.0.1 - 2026-07-29

### Added

- Change the target language directly from a translation result and immediately translate the same displayed text again without recapturing it.
- Show loading and persistence failures beside the result language control, including an in-place retry for supported-language discovery.

### Reliability and security

- Keep picker menu interaction from triggering popup click-away dismissal or cancelling the replacement translation.
- Treat a global shortcut pressed while Verba owns keyboard focus as a toggle that closes the popup without attempting another selection capture.
- Prompt the user to select text when a source-app shortcut leaves the clipboard unchanged instead of reporting a selection timeout.
- Scope retranslation to the displayed result instead of retaining a separate long-lived copy of selected text.
- Capture rendered text when the frontmost app explicitly reports that no source field is focused, while continuing to reject secure fields and fail closed on Accessibility query errors.
- Capture selected text from accessible document surfaces that omit optional subrole metadata while continuing to reject secure and unverifiable text fields.
- Fail closed when source-field security cannot be determined and strengthen clipboard restoration against concurrent changes and partial writes.
- Tie popup focus to the capture lifecycle and harden provider output, token, transport, endpoint, settings-persistence, CI, and release checks.
- Keep Linux quality checks and the Xcode 16.4 macOS host build compatible with platform- and SDK-specific translation adapters.
- Remove obsolete internal APIs and stale localization entries without changing user-facing behavior.

## 1.0.0 - 2026-07-14

Initial direct-distribution release for macOS 15 or later on Apple silicon.

### Features

- Translate selected text with Apple's Translation framework and a configurable target language.
- Proofread selected text with the OpenAI Responses API using a user-supplied API key stored in macOS Keychain.
- Keep proofreading responses concise and use GPT-5.6 Luna after live comparison with Terra.
- Invoke both actions from configurable global shortcuts and review results in a native menu-bar popup.
- Compare proofreading corrections at a glance with word-level additions and removals highlighted inline.
- Preserve rich clipboard contents during cross-application selection capture and avoid overwriting concurrent clipboard changes.
- Provide explicit permission, disclosure, offline, cancellation, credential, provider, and recovery states.
- Copy privacy-safe support diagnostics without selected text, results, or credentials.
- Optionally launch Verba at login using macOS Service Management without a helper executable.
- Check a signed Sparkle feed manually or through opt-in periodic checks, with silent automatic installation and system profiling disabled.

### Distribution and privacy

- Distributed as an Apple-silicon Developer ID application with Hardened Runtime, notarization, and a stapled ticket.
- Translation runs through Apple's framework on the Mac; macOS may download language resources.
- Proofreading sends the selected text to OpenAI only after an explicit action and first-use disclosure. API requests set `store: false`.
- Verba has no accounts, analytics, advertising, history, or cloud synchronization. Periodic update checks are opt-in and contact GitHub without selected text or API credentials.

See [PRIVACY.md](PRIVACY.md) for complete data handling and cleanup instructions and [RELEASE_TEST_MATRIX.md](RELEASE_TEST_MATRIX.md) for release qualification status.
