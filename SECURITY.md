# Security review

This document records the release security review completed on 2026-07-14. Rerun `./scripts/security-check.sh` and review this document whenever dependencies, signing, entitlements, networking, secret storage, logging, or generated bindings change.

## Reviewed controls

| Surface | Current control |
| --- | --- |
| Dependencies | `Cargo.lock` is committed and every build/check uses `--locked`. `cargo audit` checks the current RustSec database. `cargo deny` allows only reviewed licenses and crates.io sources, rejects wildcard requirements, and reports duplicate versions. Workspace crates are private and cannot be published accidentally. The security check also requires `THIRD_PARTY_NOTICES.md` to match the locked dependency graph. |
| Network | The production OpenAI constructor is pinned to `https://api.openai.com/v1/responses`, uses the system TLS trust store, rejects redirects, and has finite connect/request timeouts. Arbitrary HTTPS endpoints require an explicitly named development constructor; plain HTTP is accepted only by the loopback testing constructor for literal IPv4/IPv6 loopback addresses. Successful provider bodies are limited to 512 KiB and error bodies to 64 KiB using declared-length and streamed-byte checks. Requests set `store: false`; selected text is sent only after the user invokes proofreading and accepts the first-use disclosure. Sparkle update checks are manual or opt-in, use the fixed HTTPS GitHub feed, add no custom feed parameters, and explicitly disable system profiling. |
| Keychain | The OpenAI API key is stored as a generic-password item identified by the permanent service `io.github.the-pavels.verba` and account `openai-api-key`. Rust and Swift expose configuration state and typed failures, never the stored value. The key is loaded only for a connection test or proofreading request. |
| Logs and metrics | The app has no production application logger. Local performance signposts contain only request IDs, fixed action/state names, and budgets. Provider bodies, selected text, result text, prompts, API keys, and Keychain errors are not logged. |
| Panics and crashes | Production panic and invariant messages are fixed strings and do not interpolate selected text or secrets. Typed boundary errors redact transport, provider, Keychain, pasteboard, and generated-bridge details. |
| Generated bindings | Swift/header/module-map files are regenerated from the locked local Rust workspace on every Xcode build. The checked patch must apply cleanly or the build fails. Generated output is ignored so stale bindings cannot be treated as source of truth. |
| Entitlements | Release builds enable Hardened Runtime and request no runtime-exception entitlements. App Sandbox is not enabled; Accessibility remains protected by the user-controlled macOS TCC permission. |
| Release symbols | Release builds disable code-coverage instrumentation. Release archives generate a dSYM while installed products enable dead-code stripping and full symbol stripping. Treat dSYMs as private release artifacts and never include them in the public application package. |
| Notarization | `notarytool` reads credentials only from a named Keychain profile. Submission results and logs are retained for audit, Apple's submitted-archive hash must match locally, and the final extracted ZIP must pass stapler, signature, and Gatekeeper validation. |

## Audit result

The 2026-07-14 scan checked 181 locked third-party crates and found no known RustSec vulnerabilities. All selected dependency licenses satisfy `deny.toml`; dependency sources resolve to crates.io, with no Git dependencies. The dependency set includes permissive MIT, Apache-2.0, Unicode-3.0, and MPL-2.0 obligations. Distribution packaging must include the resulting third-party notices.

Static review found no known path that writes an API key or selected text to logs, metrics, settings, diagnostic output, or generated bridge metadata. Selected text and API keys necessarily exist transiently in process memory while an action runs; raw crash reports, process samples, and memory dumps must therefore be treated as sensitive and reviewed before sharing.

## Accepted pre-release risks

- The legacy pre-release `com.example.Verba` Keychain item is not migrated. Development users who saved a key before the permanent identifier was selected must enter it again; no released user data depends on the placeholder service.
- The app is not sandboxed because its global shortcut and cross-application selected-text workflow still require release-distribution validation under the intended signing model. Hardened Runtime remains enabled without exception entitlements. Reassess sandbox feasibility before broad distribution.
- Rust mutex poisoning and internal invariant failures can terminate the process. Their messages are redacted, but OS crash artifacts may contain transient process data and remain sensitive.
- UniFFI currently brings two transitive `winnow` versions through its generator/parser graph. The duplicate is reported by `cargo deny`, carries no advisory, and is accepted until the upstream dependency graph converges.
- dSYMs and notarization/signing credentials are intentionally outside the app bundle. The Developer ID workflow, notarization-log integrity, stapled ticket, extracted-artifact signature, entitlement set, and Gatekeeper result have been verified with real Developer ID and Keychain-profile credentials. Public and private artifact retention, checksum publication, rollback, clean-machine installation, privacy behavior, and third-party notice publication are defined in `RELEASE_CHECKLIST.md` and `PRIVACY.md`.
