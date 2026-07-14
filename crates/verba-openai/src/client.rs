use std::{sync::Arc, time::Duration};

use futures::future::{Either, select};
use serde::Serialize;
use serde_json::Value;
use url::{Host, Url};
use verba_core::{coordinator::CancellationToken, proofreading::ProofreaderError};

use crate::transport::{HttpExecutor, HttpRequest, HttpResponse, ReqwestExecutor, TransportError};

pub const OPENAI_BASE_URL: &str = "https://api.openai.com/";
pub const DEFAULT_MODEL: &str = "gpt-5.6-luna";

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenAiConfig {
    base_url: String,
    model: String,
    connect_timeout: Duration,
    request_timeout: Duration,
}

impl OpenAiConfig {
    #[must_use]
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
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
    pub fn base_url(&self) -> &str {
        &self.base_url
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
        Self::new(OPENAI_BASE_URL, DEFAULT_MODEL)
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
    text: Option<Value>,
    max_output_tokens: Option<u32>,
}

impl ResponsesApiRequest {
    #[must_use]
    pub const fn new(input: Value) -> Self {
        Self {
            input,
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
    pub const fn text_configuration(&self) -> Option<&Value> {
        self.text.as_ref()
    }

    #[must_use]
    pub const fn max_output_tokens(&self) -> Option<u32> {
        self.max_output_tokens
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
        let endpoint = responses_endpoint(&config.base_url)?;
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
        config: OpenAiConfig,
        executor: Arc<dyn HttpExecutor>,
    ) -> Result<Self, OpenAiClientBuildError> {
        let endpoint = responses_endpoint(&config.base_url)?;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    store: bool,
}

fn responses_endpoint(base_url: &str) -> Result<Url, OpenAiClientBuildError> {
    let mut base_url = Url::parse(base_url).map_err(|_| OpenAiClientBuildError::InvalidBaseUrl)?;
    if !has_allowed_transport(&base_url)
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

fn has_allowed_transport(url: &Url) -> bool {
    if url.scheme() == "https" {
        return true;
    }
    if url.scheme() != "http" {
        return false;
    }

    match url.host() {
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        Some(Host::Domain(_)) | None => false,
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
