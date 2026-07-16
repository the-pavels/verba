use std::{sync::Arc, time::Duration};

use futures::future::{Either, select};
use serde::Serialize;
use serde_json::Value;
use url::Url;
use verba_core::{coordinator::CancellationToken, proofreading::ProofreaderError};

use crate::transport::{HttpExecutor, HttpRequest, HttpResponse, ReqwestExecutor, TransportError};

pub const OPENAI_BASE_URL: &str = "https://api.openai.com/";
pub const DEFAULT_MODEL: &str = "gpt-5.6-luna";

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const OPENAI_REQUEST_TIMEOUT_SECONDS: u64 = 120;
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(OPENAI_REQUEST_TIMEOUT_SECONDS);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenAiConfig {
    model: String,
    connect_timeout: Duration,
    request_timeout: Duration,
}

impl OpenAiConfig {
    #[must_use]
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }

    #[must_use]
    pub const fn with_timeouts(
        mut self,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Self {
        self.connect_timeout = connect_timeout;
        self.request_timeout = request_timeout;
        self
    }

    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    #[must_use]
    pub const fn connect_timeout(&self) -> Duration {
        self.connect_timeout
    }

    #[must_use]
    pub const fn request_timeout(&self) -> Duration {
        self.request_timeout
    }
}

impl Default for OpenAiConfig {
    fn default() -> Self {
        Self::new(DEFAULT_MODEL)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenAiClientBuildError {
    InvalidBaseUrl,
    EmptyModel,
    InvalidTimeout,
    Transport,
}

#[derive(Clone)]
pub struct ResponsesApiRequest {
    input: Value,
    reasoning_effort: ReasoningEffort,
    text: Option<Value>,
    max_output_tokens: Option<u32>,
}

impl ResponsesApiRequest {
    #[must_use]
    pub const fn new(input: Value, reasoning_effort: ReasoningEffort) -> Self {
        Self {
            input,
            reasoning_effort,
            text: None,
            max_output_tokens: None,
        }
    }

    #[must_use]
    pub fn with_text_configuration(mut self, text: Value) -> Self {
        self.text = Some(text);
        self
    }

    #[must_use]
    pub const fn with_max_output_tokens(mut self, max_output_tokens: u32) -> Self {
        self.max_output_tokens = Some(max_output_tokens);
        self
    }

    #[must_use]
    pub const fn input(&self) -> &Value {
        &self.input
    }

    #[must_use]
    pub const fn reasoning_effort(&self) -> ReasoningEffort {
        self.reasoning_effort
    }

    #[must_use]
    pub const fn text_configuration(&self) -> Option<&Value> {
        self.text.as_ref()
    }

    #[must_use]
    pub const fn max_output_tokens(&self) -> Option<u32> {
        self.max_output_tokens
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    None,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl ReasoningEffort {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
        }
    }
}

pub struct ResponsesApiResponse {
    body: Value,
}

impl ResponsesApiResponse {
    #[must_use]
    pub const fn new(body: Value) -> Self {
        Self { body }
    }

    #[must_use]
    pub const fn body(&self) -> &Value {
        &self.body
    }

    #[must_use]
    pub fn into_body(self) -> Value {
        self.body
    }
}

pub struct OpenAiClient {
    endpoint: Url,
    model: String,
    executor: Arc<dyn HttpExecutor>,
}

impl OpenAiClient {
    pub fn new(config: OpenAiConfig) -> Result<Self, OpenAiClientBuildError> {
        Self::with_endpoint(production_responses_endpoint()?, config)
    }

    pub fn new_for_development(
        base_url: &str,
        config: OpenAiConfig,
    ) -> Result<Self, OpenAiClientBuildError> {
        Self::with_endpoint(development_responses_endpoint(base_url)?, config)
    }

    #[doc(hidden)]
    #[cfg(any(test, debug_assertions))]
    pub fn new_for_loopback_testing(
        base_url: &str,
        config: OpenAiConfig,
    ) -> Result<Self, OpenAiClientBuildError> {
        Self::with_endpoint(loopback_responses_endpoint(base_url)?, config)
    }

    fn with_endpoint(endpoint: Url, config: OpenAiConfig) -> Result<Self, OpenAiClientBuildError> {
        let model = config.model.trim();
        if model.is_empty() {
            return Err(OpenAiClientBuildError::EmptyModel);
        }
        if config.connect_timeout.is_zero() || config.request_timeout.is_zero() {
            return Err(OpenAiClientBuildError::InvalidTimeout);
        }

        let executor = ReqwestExecutor::new(config.connect_timeout, config.request_timeout)
            .map_err(|_| OpenAiClientBuildError::Transport)?;
        Ok(Self {
            endpoint,
            model: model.to_owned(),
            executor: Arc::new(executor),
        })
    }

    pub async fn create_response(
        &self,
        api_key: &str,
        request: ResponsesApiRequest,
        cancellation: &CancellationToken,
    ) -> Result<ResponsesApiResponse, ProofreaderError> {
        if cancellation.is_cancelled() {
            return Err(ProofreaderError::Cancelled);
        }
        let api_key = api_key.trim();
        if api_key.is_empty() {
            return Err(ProofreaderError::Authentication);
        }

        let body = serde_json::to_vec(&CreateResponseBody {
            model: &self.model,
            input: &request.input,
            reasoning: ReasoningConfiguration {
                effort: request.reasoning_effort,
            },
            text: request.text.as_ref(),
            max_output_tokens: request.max_output_tokens,
            store: false,
        })
        .map_err(|_| ProofreaderError::Failed)?;
        let request = HttpRequest {
            url: self.endpoint.clone(),
            authorization: format!("Bearer {api_key}"),
            body,
        };
        let send = Box::pin(self.executor.send(request));
        let cancelled = Box::pin(cancellation.cancelled());
        let response = match select(send, cancelled).await {
            Either::Left((result, _)) => result.map_err(map_transport_error)?,
            Either::Right(((), _)) => return Err(ProofreaderError::Cancelled),
        };

        if cancellation.is_cancelled() {
            return Err(ProofreaderError::Cancelled);
        }

        decode_response(response)
    }

    #[cfg(test)]
    fn with_executor(
        endpoint: Url,
        config: OpenAiConfig,
        executor: Arc<dyn HttpExecutor>,
    ) -> Result<Self, OpenAiClientBuildError> {
        let model = config.model.trim();
        if model.is_empty() {
            return Err(OpenAiClientBuildError::EmptyModel);
        }
        if config.connect_timeout.is_zero() || config.request_timeout.is_zero() {
            return Err(OpenAiClientBuildError::InvalidTimeout);
        }

        Ok(Self {
            endpoint,
            model: model.to_owned(),
            executor,
        })
    }
}

#[derive(Serialize)]
struct CreateResponseBody<'a> {
    model: &'a str,
    input: &'a Value,
    reasoning: ReasoningConfiguration,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    store: bool,
}

#[derive(Serialize)]
struct ReasoningConfiguration {
    effort: ReasoningEffort,
}

fn production_responses_endpoint() -> Result<Url, OpenAiClientBuildError> {
    responses_endpoint(OPENAI_BASE_URL, EndpointPolicy::Production)
}

fn development_responses_endpoint(base_url: &str) -> Result<Url, OpenAiClientBuildError> {
    responses_endpoint(base_url, EndpointPolicy::Development)
}

#[cfg(any(test, debug_assertions))]
fn loopback_responses_endpoint(base_url: &str) -> Result<Url, OpenAiClientBuildError> {
    responses_endpoint(base_url, EndpointPolicy::LoopbackTest)
}

#[derive(Clone, Copy)]
enum EndpointPolicy {
    Production,
    Development,
    #[cfg(any(test, debug_assertions))]
    LoopbackTest,
}

fn responses_endpoint(
    base_url: &str,
    policy: EndpointPolicy,
) -> Result<Url, OpenAiClientBuildError> {
    let mut base_url = Url::parse(base_url).map_err(|_| OpenAiClientBuildError::InvalidBaseUrl)?;
    if !has_allowed_endpoint(&base_url, policy)
        || !base_url.username().is_empty()
        || base_url.password().is_some()
        || base_url.query().is_some()
        || base_url.fragment().is_some()
    {
        return Err(OpenAiClientBuildError::InvalidBaseUrl);
    }
    if !base_url.path().ends_with('/') {
        let path = format!("{}/", base_url.path());
        base_url.set_path(&path);
    }
    base_url
        .join("v1/responses")
        .map_err(|_| OpenAiClientBuildError::InvalidBaseUrl)
}

fn has_allowed_endpoint(url: &Url, policy: EndpointPolicy) -> bool {
    match policy {
        EndpointPolicy::Production => url.as_str() == OPENAI_BASE_URL,
        EndpointPolicy::Development => url.scheme() == "https" && url.host().is_some(),
        #[cfg(any(test, debug_assertions))]
        EndpointPolicy::LoopbackTest => {
            if url.scheme() != "http" {
                return false;
            }
            match url.host() {
                Some(url::Host::Ipv4(address)) => address.is_loopback(),
                Some(url::Host::Ipv6(address)) => address.is_loopback(),
                Some(url::Host::Domain(_)) | None => false,
            }
        }
    }
}

fn decode_response(response: HttpResponse) -> Result<ResponsesApiResponse, ProofreaderError> {
    if !(200..300).contains(&response.status) {
        return Err(map_http_error(response.status, &response.body));
    }

    let body =
        serde_json::from_slice(&response.body).map_err(|_| ProofreaderError::MalformedResponse)?;
    Ok(ResponsesApiResponse { body })
}

fn map_transport_error(error: TransportError) -> ProofreaderError {
    match error {
        TransportError::Offline => ProofreaderError::Offline,
        TransportError::TimedOut => ProofreaderError::TimedOut,
        TransportError::ResponseTooLarge => ProofreaderError::ResponseTooLarge,
        TransportError::Failed => ProofreaderError::Failed,
    }
}

fn map_http_error(status: u16, body: &[u8]) -> ProofreaderError {
    match status {
        401 | 403 => ProofreaderError::Authentication,
        408 => ProofreaderError::TimedOut,
        429 if is_quota_error(body) => ProofreaderError::QuotaExceeded,
        429 => ProofreaderError::RateLimited,
        500..=599 => ProofreaderError::ServiceUnavailable,
        _ => ProofreaderError::Failed,
    }
}

fn is_quota_error(body: &[u8]) -> bool {
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|body| body.get("error")?.get("code")?.as_str().map(str::to_owned))
        .is_some_and(|code| code == "insufficient_quota")
}

#[cfg(test)]
mod tests;
