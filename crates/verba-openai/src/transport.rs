use std::time::Duration;

use reqwest::{Client, header, redirect::Policy};
use tokio::{runtime::Runtime, task::AbortHandle};
use url::Url;

pub(crate) struct HttpRequest {
    pub url: Url,
    pub authorization: String,
    pub body: Vec<u8>,
}

pub(crate) struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransportError {
    Offline,
    TimedOut,
    Failed,
}

#[async_trait::async_trait]
pub(crate) trait HttpExecutor: Send + Sync {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError>;
}

pub(crate) struct ReqwestExecutor {
    client: Client,
    runtime: Option<Runtime>,
}

impl ReqwestExecutor {
    pub fn new(
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, TransportError> {
        let client = Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .redirect(Policy::none())
            .build()
            .map_err(|_| TransportError::Failed)?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|_| TransportError::Failed)?;

        Ok(Self {
            client,
            runtime: Some(runtime),
        })
    }
}

#[async_trait::async_trait]
impl HttpExecutor for ReqwestExecutor {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let client = self.client.clone();
        let task = self
            .runtime
            .as_ref()
            .expect("the runtime is available until the executor is dropped")
            .spawn(async move {
                let response = client
                    .post(request.url)
                    .header(header::AUTHORIZATION, request.authorization)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(request.body)
                    .send()
                    .await
                    .map_err(map_error)?;
                let status = response.status().as_u16();
                let body = response.bytes().await.map_err(map_error)?.to_vec();

                Ok(HttpResponse { status, body })
            });
        let mut abort_on_drop = AbortOnDrop::new(task.abort_handle());
        let result = task.await.map_err(|_| TransportError::Failed)?;
        abort_on_drop.disarm();
        result
    }
}

impl Drop for ReqwestExecutor {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

fn map_error(error: reqwest::Error) -> TransportError {
    if error.is_timeout() {
        TransportError::TimedOut
    } else if error.is_connect() {
        TransportError::Offline
    } else {
        TransportError::Failed
    }
}

struct AbortOnDrop(Option<AbortHandle>);

impl AbortOnDrop {
    fn new(handle: AbortHandle) -> Self {
        Self(Some(handle))
    }

    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}
