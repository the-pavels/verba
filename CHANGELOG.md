# Changelog

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
