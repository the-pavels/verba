# Verba OpenAI adapter

The production default is `gpt-5.6-luna`. It was selected on 2026-07-14 after a live comparison against Terra covered no-change, multilingual, punctuation, paragraph, list, quotation, and formatting-preservation cases. Both models produced semantically correct results across the suite; Luna was slightly faster and about 49% cheaper in the sampled requests. Verba keeps the model injectable so tests can use a fixed value and the production choice can be updated independently of the transport.

The client uses the Responses API and sends `store: false`. It does not log API keys, selected text, corrected text, raw request bodies, or raw response bodies.

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

The report contains stable case IDs, outcomes, invariant pass/fail values, latency, token counts, and calculated cost. It never contains corpus text, corrected text, API keys, or provider response bodies. Release qualification requires every mandatory case and at least 90% of the full corpus to pass.

- [OpenAI model guidance](https://developers.openai.com/api/docs/models)
- [Responses API create reference](https://developers.openai.com/api/reference/resources/responses/methods/create)
