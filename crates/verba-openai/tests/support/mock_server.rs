use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Receiver},
    thread::{self, JoinHandle},
    time::Duration,
};

use serde_json::Value;

pub struct MockResponse {
    status: u16,
    body: Vec<u8>,
}

impl MockResponse {
    pub fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            body: serde_json::to_vec(&body).expect("mock response should serialize"),
        }
    }
}

pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Value,
}

pub struct MockServer {
    base_url: String,
    request: Receiver<RecordedRequest>,
    worker: JoinHandle<()>,
}

impl MockServer {
    pub fn start(response: MockResponse) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock server should bind");
        let address = listener
            .local_addr()
            .expect("mock server should have an address");
        let (sender, request) = mpsc::channel();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener
                .accept()
                .expect("mock server should accept a request");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("mock server should set a read timeout");
            let recorded = read_request(&mut stream);
            sender
                .send(recorded)
                .expect("mock request receiver should remain available");
            write_response(&mut stream, response);
        });

        Self {
            base_url: format!("http://{address}/"),
            request,
            worker,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn received(self) -> RecordedRequest {
        let request = self
            .request
            .recv_timeout(Duration::from_secs(5))
            .expect("mock server should receive a request");
        self.worker.join().expect("mock server should stop cleanly");
        request
    }
}

fn read_request(stream: &mut TcpStream) -> RecordedRequest {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let header_end = loop {
        let read = stream
            .read(&mut buffer)
            .expect("mock server should read the request");
        assert_ne!(read, 0, "request ended before headers were complete");
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };

    let header_text =
        std::str::from_utf8(&bytes[..header_end]).expect("request headers should be valid UTF-8");
    let mut lines = header_text.split("\r\n");
    let mut request_line = lines
        .next()
        .expect("request should contain a request line")
        .split_whitespace();
    let method = request_line
        .next()
        .expect("request should contain a method")
        .to_owned();
    let path = request_line
        .next()
        .expect("request should contain a path")
        .to_owned();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect::<HashMap<_, _>>();
    let content_length = headers
        .get("content-length")
        .expect("request should contain Content-Length")
        .parse::<usize>()
        .expect("Content-Length should be a number");

    while bytes.len() < header_end + content_length {
        let read = stream
            .read(&mut buffer)
            .expect("mock server should read the request body");
        assert_ne!(read, 0, "request body ended before Content-Length");
        bytes.extend_from_slice(&buffer[..read]);
    }
    let body = serde_json::from_slice(&bytes[header_end..header_end + content_length])
        .expect("request body should contain JSON");

    RecordedRequest {
        method,
        path,
        headers,
        body,
    }
}

fn write_response(stream: &mut TcpStream, response: MockResponse) {
    let reason = match response.status {
        200 => "OK",
        401 => "Unauthorized",
        429 => "Too Many Requests",
        503 => "Service Unavailable",
        _ => "Mock Response",
    };
    let headers = format!(
        "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        response.body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .expect("mock server should write response headers");
    stream
        .write_all(&response.body)
        .expect("mock server should write response body");
}
