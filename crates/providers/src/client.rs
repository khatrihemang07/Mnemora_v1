//! The HTTP(S) client every adapter shares: `ureq` on the rustls stack the app already uses (ring
//! provider, no OpenSSL, no native TLS). It sends the request, hands back the reply unread, and
//! maps transport failures onto a `Failure`.

use std::time::Duration;

use mnemora_harness::Failure;
use serde_json::Value;
use ureq::Agent;
use ureq::http::Response;
use ureq::tls::{Certificate, RootCerts, TlsConfig};

/// How long a connection may take to open.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How long a Provider may take to answer, from the request to the last byte of the reply.
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(120);

/// Largest reply read in one go (a whole answer, or a model list).
const MAX_REPLY_BYTES: u64 = 16 * 1024 * 1024;

/// Timeouts and trust for one client.
#[derive(Debug, Clone)]
pub struct Options {
    /// The time allowed for the whole reply.
    pub reply: Duration,
    /// Certificates to trust for `https` instead of the built-in Mozilla roots. Tests use it for
    /// a self-signed server; the app never sets it.
    pub roots: Option<Vec<Vec<u8>>>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            reply: REPLY_TIMEOUT,
            roots: None,
        }
    }
}

/// A blocking HTTP client with the app's timeouts.
#[derive(Clone)]
pub struct Client {
    agent: Agent,
}

impl Client {
    /// A client with the app's timeouts and the built-in roots.
    pub fn new() -> Self {
        Self::with_options(&Options::default())
    }

    /// A client built from `options`.
    pub fn with_options(options: &Options) -> Self {
        let roots = match &options.roots {
            Some(ders) => RootCerts::new_with_certs(
                &ders
                    .iter()
                    .map(|der| Certificate::from_der(der).to_owned())
                    .collect::<Vec<_>>(),
            ),
            None => RootCerts::WebPki,
        };
        let config = Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(options.reply))
            .http_status_as_error(false)
            .tls_config(TlsConfig::builder().root_certs(roots).build())
            .build();
        Self {
            agent: config.into(),
        }
    }

    /// POSTs `body` as JSON, with a bearer key when there is one. Any status comes back as a reply.
    pub fn post_json(
        &self,
        url: &str,
        body: &Value,
        key: Option<&str>,
    ) -> Result<Response<ureq::Body>, Failure> {
        let headers: Vec<(&str, String)> = key
            .map(|key| ("Authorization", format!("Bearer {key}")))
            .into_iter()
            .collect();
        self.post_with(url, body, &headers)
    }

    /// POSTs `body` as JSON with `headers` added (a provider's own key and version headers).
    pub fn post_with(
        &self,
        url: &str,
        body: &Value,
        headers: &[(&str, String)],
    ) -> Result<Response<ureq::Body>, Failure> {
        let text = serde_json::to_string(body).map_err(|_| Failure::Unexpected)?;
        let mut request = self
            .agent
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");
        for (name, value) in headers {
            request = request.header(*name, value.as_str());
        }
        request.send(text).map_err(transport_failure)
    }

    /// GETs `url`, with a bearer key when there is one. Any status comes back as a reply.
    pub fn get(&self, url: &str, key: Option<&str>) -> Result<Response<ureq::Body>, Failure> {
        let headers: Vec<(&str, String)> = key
            .map(|key| ("Authorization", format!("Bearer {key}")))
            .into_iter()
            .collect();
        self.get_with(url, &headers)
    }

    /// GETs `url` with `headers` added. Any status comes back as a reply.
    pub fn get_with(
        &self,
        url: &str,
        headers: &[(&str, String)],
    ) -> Result<Response<ureq::Body>, Failure> {
        let mut request = self.agent.get(url).header("Accept", "application/json");
        for (name, value) in headers {
            request = request.header(*name, value.as_str());
        }
        request.call().map_err(transport_failure)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

/// Reads a whole reply as text, within the size limit.
pub fn read_body(response: Response<ureq::Body>) -> Result<String, Failure> {
    response
        .into_body()
        .with_config()
        .limit(MAX_REPLY_BYTES)
        .read_to_string()
        .map_err(|_| Failure::BadResponse)
}

/// Maps a transport error to the failure the person reads. The error's own text is dropped.
pub fn transport_failure(error: ureq::Error) -> Failure {
    match error {
        ureq::Error::Timeout(_) => Failure::Timeout,
        ureq::Error::HostNotFound => Failure::Offline,
        ureq::Error::ConnectionFailed
        | ureq::Error::Io(_)
        | ureq::Error::Tls(_)
        | ureq::Error::Rustls(_) => Failure::Unreachable,
        ureq::Error::Protocol(_) | ureq::Error::LargeResponseHeader(..) => Failure::BadResponse,
        _ => Failure::Unexpected,
    }
}
