use std::{
    collections::VecDeque,
    future::pending,
    sync::{Arc, Mutex, mpsc},
    thread,
};

use futures::executor::block_on;
use serde_json::{Value, json};

use super::*;

#[test]
fn production_configuration_uses_the_documented_model_and_finite_timeouts() {
    let config = OpenAiConfig::default();

    assert_eq!(config.base_url(), OPENAI_BASE_URL);
    assert_eq!(config.model(), DEFAULT_MODEL);
    assert!(config.connect_timeout() > Duration::ZERO);
    assert!(config.request_timeout() > config.connect_timeout());
}

#[test]
fn builds_a_responses_request_without_retaining_server_state() {
    let executor = Arc::new(FakeExecutor::new(Ok(success_response(json!({
        "id": "resp_test"
    })))));
    let client = test_client(executor.clone());
    let input = json!([{
        "role": "user",
        "content": [{"type": "input_text", "text": "Private text"}]
    }]);
    let text = json!({"format": {"type": "json_schema"}});

    let response = block_on(client.create_response(
        "secret-key",
        ResponsesApiRequest::new(input.clone()).with_text_configuration(text.clone()),
        &CancellationToken::default(),
    ))
    .unwrap();

    assert_eq!(response.body(), &json!({"id": "resp_test"}));
    let requests = executor.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url, "https://example.test/openai/v1/responses");
    assert_eq!(requests[0].authorization, "Bearer secret-key");
    assert_eq!(
        requests[0].body,
        json!({
            "model": "test-model",
            "input": input,
            "text": text,
            "store": false
        })
    );
}

#[test]
fn rejects_invalid_configuration_before_building_the_transport() {
    let executor = Arc::new(FakeExecutor::new(Ok(success_response(json!({})))));

    assert_eq!(
        OpenAiClient::with_executor(OpenAiConfig::new("not a URL", "model"), executor.clone())
            .err(),
        Some(OpenAiClientBuildError::InvalidBaseUrl)
    );
    assert_eq!(
        OpenAiClient::with_executor(OpenAiConfig::new("https://example.test", "  "), executor)
            .err(),
        Some(OpenAiClientBuildError::EmptyModel)
    );
}

#[test]
fn rejects_missing_credentials_without_executing_a_request() {
    let executor = Arc::new(FakeExecutor::new(Ok(success_response(json!({})))));
    let client = test_client(executor.clone());

    assert_eq!(
        block_on(client.create_response(
            "  ",
            ResponsesApiRequest::new(json!("Text")),
            &CancellationToken::default(),
        ))
        .err(),
        Some(ProofreaderError::Authentication)
    );
    assert!(executor.requests().is_empty());
}

#[test]
fn maps_redacted_transport_and_http_failures() {
    for (response, expected) in [
        (Err(TransportError::Offline), ProofreaderError::Offline),
        (Err(TransportError::TimedOut), ProofreaderError::TimedOut),
        (
            Ok(HttpResponse {
                status: 401,
                body: b"credential details".to_vec(),
            }),
            ProofreaderError::Authentication,
        ),
        (
            Ok(HttpResponse {
                status: 429,
                body: br#"{"error":{"code":"rate_limit_exceeded"}}"#.to_vec(),
            }),
            ProofreaderError::RateLimited,
        ),
        (
            Ok(HttpResponse {
                status: 429,
                body: br#"{"error":{"code":"insufficient_quota"}}"#.to_vec(),
            }),
            ProofreaderError::QuotaExceeded,
        ),
        (
            Ok(HttpResponse {
                status: 503,
                body: b"service details".to_vec(),
            }),
            ProofreaderError::ServiceUnavailable,
        ),
    ] {
        let client = test_client(Arc::new(FakeExecutor::new(response)));

        assert_eq!(
            block_on(client.create_response(
                "secret-key",
                ResponsesApiRequest::new(json!("Text")),
                &CancellationToken::default(),
            ))
            .err(),
            Some(expected)
        );
    }
}

#[test]
fn rejects_malformed_success_bodies() {
    let client = test_client(Arc::new(FakeExecutor::new(Ok(HttpResponse {
        status: 200,
        body: b"not json".to_vec(),
    }))));

    assert_eq!(
        block_on(client.create_response(
            "secret-key",
            ResponsesApiRequest::new(json!("Text")),
            &CancellationToken::default(),
        ))
        .err(),
        Some(ProofreaderError::MalformedResponse)
    );
}

#[test]
fn cancellation_drops_an_in_flight_transport_request() {
    let (started_sender, started_receiver) = mpsc::channel();
    let (dropped_sender, dropped_receiver) = mpsc::channel();
    let executor = Arc::new(PendingExecutor {
        started: Mutex::new(Some(started_sender)),
        dropped: Mutex::new(Some(dropped_sender)),
    });
    let client = test_client(executor);
    let cancellation = CancellationToken::default();
    let cancellation_source = cancellation.clone();
    let worker = thread::spawn(move || {
        block_on(client.create_response(
            "secret-key",
            ResponsesApiRequest::new(json!("Text")),
            &cancellation,
        ))
    });

    started_receiver.recv().unwrap();
    cancellation_source.cancel();

    assert_eq!(
        worker.join().unwrap().err(),
        Some(ProofreaderError::Cancelled)
    );
    dropped_receiver.recv().unwrap();
}

fn test_client(executor: Arc<dyn HttpExecutor>) -> OpenAiClient {
    OpenAiClient::with_executor(
        OpenAiConfig::new("https://example.test/openai", "test-model"),
        executor,
    )
    .unwrap()
}

fn success_response(body: Value) -> HttpResponse {
    HttpResponse {
        status: 200,
        body: serde_json::to_vec(&body).unwrap(),
    }
}

struct RecordedRequest {
    url: String,
    authorization: String,
    body: Value,
}

struct FakeExecutor {
    responses: Mutex<VecDeque<Result<HttpResponse, TransportError>>>,
    requests: Mutex<Vec<RecordedRequest>>,
}

impl FakeExecutor {
    fn new(response: Result<HttpResponse, TransportError>) -> Self {
        Self {
            responses: Mutex::new(VecDeque::from([response])),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> std::sync::MutexGuard<'_, Vec<RecordedRequest>> {
        self.requests.lock().unwrap()
    }
}

#[async_trait::async_trait]
impl HttpExecutor for FakeExecutor {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.requests.lock().unwrap().push(RecordedRequest {
            url: request.url.to_string(),
            authorization: request.authorization,
            body: serde_json::from_slice(&request.body).unwrap(),
        });
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("a response should be configured")
    }
}

struct PendingExecutor {
    started: Mutex<Option<mpsc::Sender<()>>>,
    dropped: Mutex<Option<mpsc::Sender<()>>>,
}

#[async_trait::async_trait]
impl HttpExecutor for PendingExecutor {
    async fn send(&self, _request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let _drop_signal = DropSignal(
            self.dropped
                .lock()
                .unwrap()
                .take()
                .expect("the executor should run once"),
        );
        self.started
            .lock()
            .unwrap()
            .take()
            .expect("the executor should run once")
            .send(())
            .unwrap();
        pending().await
    }
}

struct DropSignal(mpsc::Sender<()>);

impl Drop for DropSignal {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}
