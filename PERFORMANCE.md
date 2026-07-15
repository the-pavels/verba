# Performance checks

Verba emits local Points of Interest signposts under the app bundle identifier in the `Performance` category. The signposts contain request IDs, action names, presentation-state names, and budgets only. They never contain selected text, results, prompts, credentials, or error messages, and they are not sent to an analytics service.

Measure a release build with Instruments on the oldest Mac in the supported release hardware matrix. Use at least 20 cold launches and 20 invocations per action, and require the p95 measurement to meet these budgets:

| Measurement | Signposts | p95 budget |
| --- | --- | ---: |
| App initialization | `Startup` interval | 750 ms |
| Shortcut feedback | `TextAction` start to `PopupPresented` with `state=loading` | 100 ms |
| Selected-text capture | `TextAction` start to `CaptureCompleted` | 650 ms |
| Result presentation overhead | `ProcessingCompleted` to terminal `PopupPresented` | 50 ms |

The capture budget includes the bounded 500 ms synthetic-copy timeout. Translation and OpenAI provider time is intentionally outside the result-presentation budget. Inspect it as the span between `CaptureCompleted` and `ProcessingCompleted` without treating network latency as UI overhead. The automated test suite verifies milestone ordering and metadata privacy, while release qualification supplies the hardware measurements required by the phase exit criterion.
