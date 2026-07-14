# Verba OpenAI adapter

The production default is `gpt-5.6-luna`. It was selected on 2026-07-14 after a live comparison against Terra covered no-change, multilingual, punctuation, paragraph, list, quotation, and formatting-preservation cases. Both models produced semantically correct results across the suite; Luna was slightly faster and about 49% cheaper in the sampled requests. Verba keeps the model injectable so tests can use a fixed value and the production choice can be updated independently of the transport.

The client uses the Responses API and sends `store: false`. It does not log API keys, selected text, corrected text, raw request bodies, or raw response bodies.

Contract and prompt regression tests use a local mock server and never call OpenAI. The paid live smoke test is ignored by default and also requires an explicit opt-in:

```sh
VERBA_RUN_LIVE_OPENAI_TEST=1 OPENAI_API_KEY=... cargo test -p verba-openai --test live_smoke -- --ignored
```

- [OpenAI model guidance](https://developers.openai.com/api/docs/models)
- [Responses API create reference](https://developers.openai.com/api/reference/resources/responses/methods/create)
