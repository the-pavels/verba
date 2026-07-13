# Verba OpenAI adapter

The production default is `gpt-5.6-terra`. It was selected on 2026-07-13 because OpenAI's current model guidance recommends Terra when balancing intelligence and cost. Verba keeps the model injectable so tests can use a fixed value and the production choice can be updated independently of the transport.

The client uses the Responses API and sends `store: false`. It does not log API keys, selected text, corrected text, raw request bodies, or raw response bodies.

- [OpenAI model guidance](https://developers.openai.com/api/docs/models)
- [Responses API create reference](https://developers.openai.com/api/reference/resources/responses/methods/create)
