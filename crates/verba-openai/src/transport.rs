use std::time::Duration;

use reqwest::{Client, header, redirect::Policy};
use tokio::{runtime::Runtime, task::AbortHandle};
use url::Url;

pub(crate) const MAX_SUCCESS_RESPONSE_BYTES: usize = 512 * 1024;
pub(crate) const MAX_ERROR_RESPONSE_BYTES: usize = 64 * 1024;

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
    ResponseTooLarge,
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
                let mut response = client
                    .post(request.url)
                    .header(header::AUTHORIZATION, request.authorization)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(request.body)
                    .send()
                    .await
                    .map_err(map_error)?;
                let status = response.status().as_u16();
                let body = read_bounded_body(&mut response, response_limit(status)).await?;

                Ok(HttpResponse { status, body })
            });
        let mut abort_on_drop = AbortOnDrop::new(task.abort_handle());
        let result = task.await.map_err(|_| TransportError::Failed)?;
        abort_on_drop.disarm();
        result
    }
}

const fn response_limit(status: u16) -> usize {
    if status >= 200 && status < 300 {
        MAX_SUCCESS_RESPONSE_BYTES
    } else {
        MAX_ERROR_RESPONSE_BYTES
    }
}

async fn read_bounded_body(
    response: &mut reqwest::Response,
    maximum_bytes: usize,
) -> Result<Vec<u8>, TransportError> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum_bytes as u64)
    {
        return Err(TransportError::ResponseTooLarge);
    }

    let mut body = Vec::with_capacity(maximum_bytes);
    while let Some(chunk) = response.chunk().await.map_err(map_error)? {
        if chunk.len() > maximum_bytes.saturating_sub(body.len()) {
            return Err(TransportError::ResponseTooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
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

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    use futures::executor::block_on;

    use super::*;

    #[test]
    fn rejects_oversized_success_and_error_bodies_from_headers_or_streaming() {
        for (status, maximum_bytes) in [
            (200, MAX_SUCCESS_RESPONSE_BYTES),
            (400, MAX_ERROR_RESPONSE_BYTES),
        ] {
            let (url, server) = response_server(status, Vec::new(), Some(maximum_bytes + 1), None);
            assert!(matches!(send(url), Err(TransportError::ResponseTooLarge)));
            server.join().unwrap();

            let (url, server) = response_server(status, vec![b'x'; maximum_bytes + 1], None, None);
            assert!(matches!(send(url), Err(TransportError::ResponseTooLarge)));
            server.join().unwrap();
        }
    }

    #[test]
    fn accepts_bodies_exactly_at_each_limit() {
        for (status, maximum_bytes) in [
            (200, MAX_SUCCESS_RESPONSE_BYTES),
            (400, MAX_ERROR_RESPONSE_BYTES),
        ] {
            let body = vec![b'x'; maximum_bytes];
            let (url, server) = response_server(status, body, Some(maximum_bytes), None);
            let response = send(url).expect("a body exactly at the limit should be accepted");
            assert_eq!(response.status, status);
            assert_eq!(response.body.len(), maximum_bytes);
            server.join().unwrap();
        }
    }

    #[test]
    fn rejects_redirects_without_forwarding_authorization() {
        let target = TcpListener::bind("127.0.0.1:0").unwrap();
        target.set_nonblocking(true).unwrap();
        let location = format!("http://{}/capture", target.local_addr().unwrap());
        let (url, server) = response_server(302, Vec::new(), Some(0), Some(&location));

        let response = send(url).expect("redirect responses should be returned without following");

        assert_eq!(response.status, 302);
        server.join().unwrap();
        thread::sleep(Duration::from_millis(50));
        assert!(matches!(
            target.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
    }

    fn send(url: Url) -> Result<HttpResponse, TransportError> {
        let executor = ReqwestExecutor::new(Duration::from_secs(1), Duration::from_secs(5))?;
        block_on(executor.send(HttpRequest {
            url,
            authorization: "Bearer private-test-key".to_owned(),
            body: b"{}".to_vec(),
        }))
    }

    fn response_server(
        status: u16,
        body: Vec<u8>,
        content_length: Option<usize>,
        location: Option<&str>,
    ) -> (Url, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = Url::parse(&format!(
            "http://{}/v1/responses",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let location = location.map(str::to_owned);
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
            }

            let reason = if status == 200 { "OK" } else { "Test Response" };
            let mut headers = format!("HTTP/1.1 {status} {reason}\r\nConnection: close\r\n");
            if let Some(content_length) = content_length {
                headers.push_str(&format!("Content-Length: {content_length}\r\n"));
            }
            if let Some(location) = location {
                headers.push_str(&format!("Location: {location}\r\n"));
            }
            headers.push_str("\r\n");
            let _ = stream.write_all(headers.as_bytes());
            let _ = stream.write_all(&body);
        });
        (url, server)
    }
}
