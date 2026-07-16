# Privacy and data handling

This document describes Verba 1.0.0. Verba has no accounts, analytics, advertising, telemetry, or cloud synchronization operated by the Verba project.

## What each action does

| Action | Data used | Where processing happens |
| --- | --- | --- |
| Translate | The selected plain text and chosen target language | Apple's Translation framework on the Mac. macOS may download language resources managed by Apple. Verba does not send translation text to a Verba-operated server. |
| Proofread | The selected plain text, fixed proofreading instructions, and a strict response schema | OpenAI's Responses API at `https://api.openai.com/v1/responses`, using the API key configured by the user. The action runs only after an explicit Proofread command and first-use disclosure. |

Verba does not automatically replace text. It displays the result and lets the user copy it.

## Selection capture and Accessibility

Verba asks for macOS Accessibility access only after the user explicitly starts the permission flow. It uses that permission to send a synthetic Copy command to the frontmost application so it can read the current selection. Verba does not request Apple Events, Automation, or Input Monitoring permission.

Selection capture is a bounded clipboard transaction:

1. Verba snapshots the current pasteboard.
2. It sends Copy and waits up to 500 milliseconds for plain text.
3. It restores the snapshot only when no other process changed the pasteboard in the meantime.

If another clipboard change wins the race, Verba cancels restoration instead of overwriting newer content. Before clearing the clipboard for restoration, Verba builds every replacement item in memory. After writing, it verifies the change count, item count, and original representation types. It retries once only when the same clipboard ownership is still current and the failed write left it empty.

macOS does not provide an atomic multi-item clipboard replacement operation: clearing and writing are separate calls. A rare system write failure after the clear can therefore leave the clipboard empty or partially restored. Verba reports the failure and does not retry over a newer clipboard value. Empty selections, unsupported content, secure text fields, and fields whose security cannot be verified are rejected. The selected text and result exist transiently in process memory while the action runs.

## Proofreading and OpenAI

Proofreading sends the selected text to OpenAI under the user's own API account. Verba sets `store: false` and does not create a conversation or retain a response for later retrieval. This setting does not mean that OpenAI never retains request data: OpenAI documents that API inputs and outputs may be included in abuse-monitoring logs for up to 30 days by default, unless the API organization has an approved data-retention control. OpenAI states that API data is not used to train its models unless the customer explicitly opts in. See [OpenAI's API data controls](https://developers.openai.com/api/docs/guides/your-data) for the current provider policy.

Verba rejects redirects, requires HTTPS for production requests, and applies finite connection and request timeouts. It does not log request bodies, response bodies, authorization headers, selected text, corrected text, or API keys.

## Launch at login

Launch at login is off by default. If the user enables **Launch Verba at login** in Settings, Verba registers its main application with macOS using `SMAppService.mainApp`. macOS owns the registration and any required approval in **System Settings > General > Login Items**. Verba does not install a separate login helper or mirror this state in its preferences.

The user can turn the setting off in Verba or disable it in System Settings. Unregistering prevents Verba from launching at future logins but does not terminate a currently running copy.

## Updates and GitHub

Verba uses Sparkle to check a signed update feed hosted on GitHub. It makes no update request until the user chooses **Check for Updates…** or enables **Check for updates automatically** in Settings. Automatic checks are off by default and can be disabled from the same setting. When enabled, Sparkle uses its default scheduled interval of approximately one day while the app is running.

An update check requests `https://github.com/the-pavels/verba/releases/latest/download/appcast.xml` over HTTPS. GitHub necessarily receives the connecting IP address, request time, and ordinary HTTPS request metadata. Verba does not add selected text, clipboard contents, the OpenAI API key, proofreading data, or custom feed parameters. Sparkle system profiling is explicitly disabled, so Verba does not append Sparkle's optional operating-system, hardware, application, or language profile to the feed URL. See [Sparkle's update settings](https://sparkle-project.org/documentation/customization/) and [system-profiling documentation](https://sparkle-project.org/documentation/system-profiling/).

If an update is available, Sparkle displays it for the user to review. Verba disables silent automatic update installation. Accepted update archives are downloaded from the matching GitHub release and must pass the app's Developer ID checks plus the signed-feed and Ed25519 archive checks before installation.

## Data stored on the Mac

Verba stores these non-secret preferences in the macOS preferences domain `io.github.the-pavels.verba`:

- target translation language;
- acknowledgement of the proofreading disclosure;
- Translate and Proofread shortcuts;
- whether the Accessibility permission explanation has been requested before;
- Sparkle's automatic-check preference and local update-check state.

Launch-at-login state is held by macOS Service Management rather than the Verba preferences domain.

The OpenAI API key is a macOS Keychain generic-password item with service `io.github.the-pavels.verba` and account `openai-api-key`. Verba reads it only when testing the OpenAI connection or performing proofreading. Translation does not need the key.

Support diagnostics show only the app version, macOS version, architecture, Accessibility state, target language, shortcut settings, whether an API key is configured, and a safe error code. They exclude document content, selected text, results, the API key, and provider response bodies. Raw macOS crash reports, process samples, and memory dumps can contain transient process data and should be reviewed as sensitive before sharing.

## Uninstall and complete local cleanup

Removing `Verba.app` stops the application but intentionally does not delete its Keychain item, preferences, macOS Accessibility decision, or Service Management state. This lets a normal reinstall preserve the user's settings and key.

For a complete cleanup:

1. In Verba Settings, turn off **Launch Verba at login**, then quit Verba. If the app was already removed or the registration still appears, disable Verba in **System Settings > General > Login Items**. macOS can retain a stale visual entry temporarily after the app is removed.
2. In Verba Settings, choose **Delete API Key**, then move `Verba.app` to the Trash. If the app was already removed, delete the Keychain item in Keychain Access, or run:

   ```sh
   security delete-generic-password -s io.github.the-pavels.verba -a openai-api-key
   ```

3. Remove non-secret preferences:

   ```sh
   defaults delete io.github.the-pavels.verba
   ```

4. In System Settings, open **Privacy & Security > Accessibility** and remove Verba. The equivalent terminal reset is:

   ```sh
   tccutil reset Accessibility io.github.the-pavels.verba
   ```

The commands can report that no matching item or domain exists when cleanup was already complete.

## Current limitations

- macOS 15 or later and Apple silicon (`arm64`) are required. Intel Macs and the Mac App Store are not supported.
- Only plain-text selections up to 10,000 characters are accepted.
- Global shortcuts default to Control-Option-T for Translate and Control-Option-P for Proofread and can be changed in Settings.
- Accessibility permission is required for cross-application selection capture. Some applications or protected fields may prevent capture.
- Translation availability depends on Apple's supported languages and any required language-resource download.
- Proofreading requires network access, a valid OpenAI API key, available API quota, and OpenAI service availability.
- Launch at login and periodic update checks are optional and off by default. Silent automatic update installation is disabled.
- Verba does not replace selected text automatically, keep history, or synchronize settings.

Privacy behavior should be reviewed again whenever capture, networking, storage, diagnostics, or providers change.
