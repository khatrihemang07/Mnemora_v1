//! An in-process HTTP server on loopback. It records each request and answers with a canned
//! reply, so the adapters run against the real wire protocol with no network and no Model.

#![allow(clippy::expect_used)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// One request the server received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    /// `GET`, `POST` and so on.
    pub method: String,
    /// The path and query, as sent (`/v1/chat/completions`).
    pub path: String,
    /// The `Authorization` header, if sent.
    pub authorization: Option<String>,
    /// The request body as text.
    pub body: String,
}

/// The reply the server gives: a status, a body and how long it waits before sending it.
#[derive(Debug, Clone)]
pub struct Canned {
    /// The HTTP status code.
    pub status: u16,
    /// The `Content-Type`.
    pub content_type: &'static str,
    /// The body, sent as is.
    pub body: String,
    /// How long to wait before answering.
    pub delay: Duration,
}

impl Canned {
    /// A JSON reply with `status`.
    pub fn json(status: u16, body: &str) -> Self {
        Self {
            status,
            content_type: "application/json",
            body: body.to_owned(),
            delay: Duration::ZERO,
        }
    }

    /// A server-sent events reply, status 200.
    pub fn sse(body: &str) -> Self {
        Self {
            status: 200,
            content_type: "text/event-stream",
            body: body.to_owned(),
            delay: Duration::ZERO,
        }
    }

    /// Waits `delay` before answering.
    #[must_use]
    pub fn delayed(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

/// A loopback server that answers every request with what `respond` returns for it.
pub struct FakeServer {
    addr: SocketAddr,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl FakeServer {
    /// Starts the server on a free loopback port. It runs until the test process ends.
    pub fn start<F>(respond: F) -> Self
    where
        F: Fn(&Seen) -> Canned + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("local address");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let respond = Arc::new(respond);
        let log = Arc::clone(&seen);
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let respond = Arc::clone(&respond);
                let log = Arc::clone(&log);
                thread::spawn(move || serve(stream, &*respond, &log));
            }
        });
        Self { addr, seen }
    }

    /// The base address a Provider is given, with `/v1`.
    pub fn address(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    /// Every request received so far, oldest first.
    pub fn requests(&self) -> Vec<Seen> {
        self.seen.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

/// An address with nothing listening on it: connecting is refused.
pub fn closed_address() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("local address");
    drop(listener);
    format!("http://{addr}/v1")
}

fn serve(stream: TcpStream, respond: &dyn Fn(&Seen) -> Canned, log: &Mutex<Vec<Seen>>) {
    let Some(seen) = read_request(&stream) else {
        return;
    };
    if let Ok(mut all) = log.lock() {
        all.push(seen.clone());
    }
    let canned = respond(&seen);
    thread::sleep(canned.delay);
    let reply = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        canned.status,
        reason(canned.status),
        canned.content_type,
        canned.body.len(),
        canned.body
    );
    let mut stream = stream;
    let _ = stream.write_all(reply.as_bytes());
    let _ = stream.flush();
}

fn read_request(stream: &TcpStream) -> Option<Seen> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_owned();
    let path = parts.next()?.to_owned();
    let mut length = 0usize;
    let mut authorization = None;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            let value = value.trim();
            if name.eq_ignore_ascii_case("content-length") {
                length = value.parse().unwrap_or(0);
            } else if name.eq_ignore_ascii_case("authorization") {
                authorization = Some(value.to_owned());
            }
        }
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).ok()?;
    Some(Seen {
        method,
        path,
        authorization,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        502 => "Bad Gateway",
        504 => "Gateway Timeout",
        _ => "Error",
    }
}
