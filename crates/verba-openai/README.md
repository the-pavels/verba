# Verba OpenAI adapter

The production default is `gpt-5.6-luna`. It was selected on 2026-07-14 after a live comparison against Terra covered no-change, multilingual, punctuation, paragraph, list, quotation, and formatting-preservation cases. Both models produced semantically correct results across the suite; Luna was slightly faster and about 49% cheaper in the sampled requests. Verba keeps the model injectable so tests can use a fixed value and the production choice can be updated independently of the transport.

The client uses the Responses API and sends `store: false`. It does not log API keys, selected text, corrected text, raw request bodies, or raw response bodies.

## Request policy

Request policy uses explicit `medium` reasoning effort for proofreading and `low` for connection tests. Although OpenAI recommends low effort for straightforward rewrites, Verba's release evaluation showed that low effort did not reliably follow its stricter whitespace and formatting-preservation contract. The proofreading response ceiling remains 16,384 tokens so long corrections have room for reasoning plus structured output. The connection test uses a 256-token ceiling so its strict `{"ok": true}` response has a bounded reasoning reserve. Production requests have a cancellable 120-second timeout so selections near the supported input boundary are not cut off by the previous 30-second limit.

Proofreading retains the 10,000-character limit and also applies a conservative preflight estimate of one token per UTF-8 byte, capped at 10,000 estimated tokens. This intentionally overestimates ordinary Latin text and prevents token-dense Unicode selections from reaching the network when they could consume disproportionate context. It is a local safety bound, not a claim about the provider tokenizer.

Contract and fixture-decoding tests use a local mock server and never call OpenAI. They prove request serialization, strict-schema handling, and deterministic policy validation; they do not prove how a real model behaves.

The paid live smoke test is ignored by default and requires an explicit opt-in:

```sh
VERBA_RUN_LIVE_OPENAI_TEST=1 OPENAI_API_KEY=... cargo test -p verba-openai --test live_smoke -- --ignored
```

## Release evaluation

The versioned synthetic corpus in `tests/fixtures/proofreading-evaluation-v1.json` evaluates the production model and exact production request configuration. Language, meaning, tone, and spelling/grammar scope are evaluated best effort against reference outcomes. The core mechanically rejects changes to exact leading/trailing Unicode whitespace, line endings and blank-line positions, Markdown list/blockquote/fence prefixes, inline code delimiters, and paired strong-emphasis/strikethrough markers.

Run the evaluator only when intentionally authorizing paid API calls. Supply current input and output prices per million tokens so the privacy-safe report can calculate cost without hard-coding provider pricing:

```sh
VERBA_RUN_LIVE_OPENAI_EVAL=1 \
OPENAI_API_KEY=... \
VERBA_EVAL_INPUT_USD_PER_MILLION=... \
VERBA_EVAL_OUTPUT_USD_PER_MILLION=... \
VERBA_EVAL_REPORT_PATH=/tmp/verba-proofreading-eval.json \
cargo test -p verba-openai production_model_meets_release_threshold -- --ignored
```

The evaluator first runs the production connection-test request five times, then runs the proofreading corpus. The report records the model, request-policy version, reasoning efforts, request timeout, configured input/output ceilings, stable case IDs, outcomes, invariant pass/fail values, latency, total output tokens, reasoning tokens, visible output tokens, and calculated cost. It never contains corpus text, corrected text, API keys, or provider response bodies. Release qualification requires all five connection attempts, every mandatory case, and at least 90% of the full corpus to pass.

- [OpenAI model guidance](https://developers.openai.com/api/docs/models)
- [Responses API create reference](https://developers.openai.com/api/reference/resources/responses/methods/create)
