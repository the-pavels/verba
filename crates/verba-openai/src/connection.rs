use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;
use verba_core::{coordinator::CancellationToken, proofreading::ProofreaderError};

use crate::{
    CONNECTION_TEST_MAX_OUTPUT_TOKENS, OpenAiClient, PROOFREADING_REASONING_EFFORT,
    ResponsesApiRequest, ResponsesApiResponse,
};

pub struct OpenAiConnectionTester {
    client: Arc<dyn ResponsesClient>,
}

impl OpenAiConnectionTester {
    #[must_use]
    pub fn new(client: Arc<OpenAiClient>) -> Self {
        Self { client }
    }

    pub async fn test(
        &self,
        api_key: &str,
        cancellation: &CancellationToken,
    ) -> Result<(), ProofreaderError> {
        let response = self
            .client
            .create_response(api_key, connection_test_request(), cancellation)
            .await?;
        decode_connection_test_response(response)
    }

    #[cfg(test)]
    fn with_client(client: Arc<dyn ResponsesClient>) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
trait ResponsesClient: Send + Sync {
    async fn create_response(
        &self,
        api_key: &str,
        request: ResponsesApiRequest,
        cancellation: &CancellationToken,
    ) -> Result<ResponsesApiResponse, ProofreaderError>;
}

#[async_trait::async_trait]
impl ResponsesClient for OpenAiClient {
    async fn create_response(
        &self,
        api_key: &str,
        request: ResponsesApiRequest,
        cancellation: &CancellationToken,
    ) -> Result<ResponsesApiResponse, ProofreaderError> {
        OpenAiClient::create_response(self, api_key, request, cancellation).await
    }
}

pub(crate) fn connection_test_request() -> ResponsesApiRequest {
    ResponsesApiRequest::new(
        json!([{
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": "Return true in the required connection-test schema."
            }]
        }]),
        PROOFREADING_REASONING_EFFORT,
    )
    .with_text_configuration(json!({
        "format": {
            "type": "json_schema",
            "name": "connection_test",
            "description": "Confirms authentication and strict schema support.",
            "strict": true,
            "schema": {
                "type": "object",
                "properties": {
                    "ok": { "type": "boolean", "const": true }
                },
                "required": ["ok"],
                "additionalProperties": false
            }
        }
    }))
    .with_max_output_tokens(CONNECTION_TEST_MAX_OUTPUT_TOKENS)
}

pub(crate) fn decode_connection_test_response(
    response: ResponsesApiResponse,
) -> Result<(), ProofreaderError> {
    let response = serde_json::from_value::<ResponseEnvelope>(response.into_body())
        .map_err(|_| ProofreaderError::MalformedResponse)?;
    match response.status {
        ResponseStatus::Completed => {}
        ResponseStatus::Incomplete | ResponseStatus::Queued | ResponseStatus::InProgress => {
            return Err(ProofreaderError::Incomplete);
        }
        ResponseStatus::Cancelled => return Err(ProofreaderError::Cancelled),
        ResponseStatus::Failed | ResponseStatus::Unknown => return Err(ProofreaderError::Failed),
    }

    let mut output_text = None;
    for output in response.output {
        let OutputItem::Message { content } = output else {
            continue;
        };
        for content in content {
            match content {
                OutputContent::OutputText { text } => {
                    if output_text.replace(text).is_some() {
                        return Err(ProofreaderError::MalformedResponse);
                    }
                }
                OutputContent::Refusal => return Err(ProofreaderError::Refused),
                OutputContent::Other => {}
            }
        }
    }

    let output_text = output_text.ok_or(ProofreaderError::MalformedResponse)?;
    let payload = serde_json::from_str::<ConnectionTestPayload>(&output_text)
        .map_err(|_| ProofreaderError::MalformedResponse)?;
    if payload.ok {
        Ok(())
    } else {
        Err(ProofreaderError::MalformedResponse)
    }
}

#[derive(Deserialize)]
struct ResponseEnvelope {
    status: ResponseStatus,
    #[serde(default)]
    output: Vec<OutputItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ResponseStatus {
    Completed,
    Incomplete,
    Queued,
    InProgress,
    Failed,
    Cancelled,
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutputItem {
    Message {
        #[serde(default)]
        content: Vec<OutputContent>,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutputContent {
    OutputText {
        text: String,
    },
    Refusal,
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConnectionTestPayload {
    ok: bool,
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use futures::executor::block_on;
    use serde_json::Value;

    use super::*;

    #[test]
    fn uses_a_fixed_minimal_strict_schema_request() {
        let client = Arc::new(FakeResponsesClient::new(Ok(success_response())));
        let tester = OpenAiConnectionTester::with_client(client.clone());

        assert_eq!(
            block_on(tester.test("test-key", &CancellationToken::default())),
            Ok(())
        );

        let calls = client.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].api_key, "test-key");
        assert_eq!(
            calls[0].request.reasoning_effort(),
            PROOFREADING_REASONING_EFFORT
        );
        assert_eq!(
            calls[0].request.max_output_tokens(),
            Some(CONNECTION_TEST_MAX_OUTPUT_TOKENS)
        );
        assert_eq!(
            calls[0].request.input(),
            &json!([{
                "role": "developer",
                "content": [{
                    "type": "input_text",
                    "text": "Return true in the required connection-test schema."
                }]
            }])
        );
        assert_eq!(
            calls[0].request.text_configuration(),
            Some(&json!({
                "format": {
                    "type": "json_schema",
                    "name": "connection_test",
                    "description": "Confirms authentication and strict schema support.",
                    "strict": true,
                    "schema": {
                        "type": "object",
                        "properties": {
                            "ok": { "type": "boolean", "const": true }
                        },
                        "required": ["ok"],
                        "additionalProperties": false
                    }
                }
            }))
        );
    }

    #[test]
    fn preserves_provider_errors_without_including_credentials() {
        for error in [
            ProofreaderError::Authentication,
            ProofreaderError::RateLimited,
            ProofreaderError::QuotaExceeded,
            ProofreaderError::Offline,
            ProofreaderError::TimedOut,
            ProofreaderError::ServiceUnavailable,
        ] {
            let tester =
                OpenAiConnectionTester::with_client(Arc::new(FakeResponsesClient::new(Err(error))));
            assert_eq!(
                block_on(tester.test("test-key", &CancellationToken::default())),
                Err(error)
            );
        }
    }

    #[test]
    fn rejects_nonconforming_success_responses() {
        for body in [
            json!({"status": "completed", "output": []}),
            response_with_text(r#"{"ok":false}"#),
            response_with_text(r#"{"ok":true,"details":"unexpected"}"#),
            json!({
                "status": "completed",
                "output": [{"type": "message", "content": [{"type": "refusal"}]}]
            }),
        ] {
            let tester = OpenAiConnectionTester::with_client(Arc::new(FakeResponsesClient::new(
                Ok(ResponsesApiResponse::new(body)),
            )));
            assert!(block_on(tester.test("test-key", &CancellationToken::default())).is_err());
        }
    }

    #[test]
    fn maps_nonterminal_and_terminal_statuses_to_actionable_errors() {
        for status in ["incomplete", "queued", "in_progress"] {
            assert_eq!(
                decode_connection_test_response(ResponsesApiResponse::new(json!({
                    "status": status,
                    "output": []
                }))),
                Err(ProofreaderError::Incomplete)
            );
        }
        assert_eq!(
            decode_connection_test_response(ResponsesApiResponse::new(json!({
                "status": "cancelled",
                "output": []
            }))),
            Err(ProofreaderError::Cancelled)
        );
        for status in ["failed", "unknown_status"] {
            assert_eq!(
                decode_connection_test_response(ResponsesApiResponse::new(json!({
                    "status": status,
                    "output": []
                }))),
                Err(ProofreaderError::Failed)
            );
        }
    }

    fn success_response() -> ResponsesApiResponse {
        ResponsesApiResponse::new(response_with_text(r#"{"ok":true}"#))
    }

    fn response_with_text(text: &str) -> Value {
        json!({
            "status": "completed",
            "output": [{
                "type": "message",
                "content": [{"type": "output_text", "text": text}]
            }]
        })
    }

    struct RecordedCall {
        api_key: String,
        request: ResponsesApiRequest,
    }

    struct FakeResponsesClient {
        result: Mutex<Option<Result<ResponsesApiResponse, ProofreaderError>>>,
        calls: Mutex<Vec<RecordedCall>>,
    }

    impl FakeResponsesClient {
        fn new(result: Result<ResponsesApiResponse, ProofreaderError>) -> Self {
            Self {
                result: Mutex::new(Some(result)),
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl ResponsesClient for FakeResponsesClient {
        async fn create_response(
            &self,
            api_key: &str,
            request: ResponsesApiRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ResponsesApiResponse, ProofreaderError> {
            self.calls.lock().unwrap().push(RecordedCall {
                api_key: api_key.to_owned(),
                request,
            });
            self.result
                .lock()
                .unwrap()
                .take()
                .expect("the connection test should execute once")
        }
    }
}
