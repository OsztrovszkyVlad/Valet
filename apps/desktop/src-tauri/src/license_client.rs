//! Online license check (`docs/LICENSING.md`, "Online check"): the only
//! network calls besides the updater. Each request is a POST with a small
//! JSON body holding `license_id`, `machine` and (activate, validate)
//! `app_version`; nothing else is sent.
//!
//! When `license/api.txt` or `license/pubkey.txt` is empty, licensing is not
//! configured: [`LicenseClient::configured`] returns
//! [`ClientError::NotConfigured`] before any HTTP client exists, so no request
//! can be made.

use std::time::Duration;

use serde::{Deserialize, Serialize};

pub(crate) const ERR_NOT_CONFIGURED: &str = "license.not_configured";
pub(crate) const ERR_NETWORK: &str = "license.network";
pub(crate) const ERR_UNKNOWN: &str = "license.unknown";
pub(crate) const ERR_TOO_MANY: &str = "license.too_many";
pub(crate) const ERR_REVOKED: &str = "license.revoked";

/// Request timeout (connect and answer).
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// The version sent as `app_version`.
pub(crate) const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientError {
    /// No API base URL or no public key: no request was made.
    NotConfigured,
    /// No answer, a timeout, a 5xx or any unexpected answer. The current
    /// token is kept.
    Network,
    /// 404: the license id is unknown.
    Unknown,
    /// 409: `max_activations` other machines are active.
    TooMany,
    /// 410: the license was revoked.
    Revoked,
}

impl ClientError {
    pub(crate) fn code(self) -> &'static str {
        match self {
            ClientError::NotConfigured => ERR_NOT_CONFIGURED,
            ClientError::Network => ERR_NETWORK,
            ClientError::Unknown => ERR_UNKNOWN,
            ClientError::TooMany => ERR_TOO_MANY,
            ClientError::Revoked => ERR_REVOKED,
        }
    }

    /// The server refused this machine or license (404, 409, 410): the
    /// stored activation token no longer applies.
    pub(crate) fn drops_token(self) -> bool {
        matches!(
            self,
            ClientError::Unknown | ClientError::TooMany | ClientError::Revoked
        )
    }
}

/// The error for an HTTP status other than 200 (activate, validate) or 204
/// (deactivate).
pub(crate) fn status_error(status: u16) -> ClientError {
    match status {
        404 => ClientError::Unknown,
        409 => ClientError::TooMany,
        410 => ClientError::Revoked,
        _ => ClientError::Network,
    }
}

#[derive(Serialize)]
struct CheckBody<'a> {
    license_id: &'a str,
    machine: &'a str,
    app_version: &'a str,
}

#[derive(Serialize)]
struct DeactivateBody<'a> {
    license_id: &'a str,
    machine: &'a str,
}

/// A 200 answer: `token` is the envelope object `{payload, signature}`.
#[derive(Deserialize)]
struct TokenAnswer {
    token: TokenEnvelope,
}

#[derive(Serialize, Deserialize)]
struct TokenEnvelope {
    payload: String,
    signature: String,
}

#[cfg(test)]
thread_local! {
    /// HTTP clients built on this thread (tests assert none is built when
    /// licensing is not configured).
    pub(crate) static CLIENTS_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) struct LicenseClient {
    http: reqwest::Client,
    base: String,
}

impl LicenseClient {
    /// `NotConfigured` unless both the API base and the public key are set;
    /// only then is an HTTP client built.
    pub(crate) fn configured(
        api_base: Option<&str>,
        public_key: Option<&str>,
        https_only: bool,
    ) -> Result<Self, ClientError> {
        match (api_base, public_key) {
            (Some(base), Some(_)) => Self::build(base, https_only, REQUEST_TIMEOUT),
            _ => Err(ClientError::NotConfigured),
        }
    }

    /// An HTTP client for `base`. `https_only = false` only in tests (local
    /// test server).
    pub(crate) fn build(
        base: &str,
        https_only: bool,
        timeout: Duration,
    ) -> Result<Self, ClientError> {
        let base = base.trim().trim_end_matches('/');
        if base.is_empty() {
            return Err(ClientError::NotConfigured);
        }
        // reqwest uses rustls without a built-in provider (as the updater).
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
        #[cfg(test)]
        CLIENTS_BUILT.with(|c| c.set(c.get() + 1));
        let mut builder = reqwest::Client::builder()
            .https_only(https_only)
            .timeout(timeout)
            .connect_timeout(timeout)
            .redirect(reqwest::redirect::Policy::none());
        if !https_only {
            // Test server on 127.0.0.1: never through a proxy.
            builder = builder.no_proxy();
        }
        let http = builder.build().map_err(|e| {
            log::error!("License client setup failed: {e}");
            ClientError::Network
        })?;
        Ok(Self {
            http,
            base: base.to_string(),
        })
    }

    /// `POST {api}/v1/activate`: the token (envelope JSON text) for this
    /// machine.
    pub(crate) async fn activate(
        &self,
        license_id: &str,
        machine: &str,
    ) -> Result<String, ClientError> {
        self.token_call("activate", license_id, machine).await
    }

    /// `POST {api}/v1/validate`: a fresh token (envelope JSON text).
    pub(crate) async fn validate(
        &self,
        license_id: &str,
        machine: &str,
    ) -> Result<String, ClientError> {
        self.token_call("validate", license_id, machine).await
    }

    /// `POST {api}/v1/deactivate`: frees this machine's slot. 204 is
    /// success (also when the machine was not active).
    pub(crate) async fn deactivate(
        &self,
        license_id: &str,
        machine: &str,
    ) -> Result<(), ClientError> {
        let res = self
            .http
            .post(format!("{}/v1/deactivate", self.base))
            .json(&DeactivateBody {
                license_id,
                machine,
            })
            .send()
            .await
            .map_err(network)?;
        match res.status().as_u16() {
            200..=299 => Ok(()),
            s => Err(status_error(s)),
        }
    }

    async fn token_call(
        &self,
        call: &str,
        license_id: &str,
        machine: &str,
    ) -> Result<String, ClientError> {
        let res = self
            .http
            .post(format!("{}/v1/{call}", self.base))
            .json(&CheckBody {
                license_id,
                machine,
                app_version: APP_VERSION,
            })
            .send()
            .await
            .map_err(network)?;
        let status = res.status().as_u16();
        if status != 200 {
            log::warn!("License {call}: HTTP {status}");
            return Err(status_error(status));
        }
        let answer: TokenAnswer = res.json().await.map_err(network)?;
        serde_json::to_string(&answer.token).map_err(|_| ClientError::Network)
    }
}

fn network(e: reqwest::Error) -> ClientError {
    // No URL in the log line: it is the license service, not user data,
    // but the error text is enough.
    log::warn!("License service unreachable: {}", e.without_url());
    ClientError::Network
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    /// A one-request HTTP server on 127.0.0.1. Returns its base URL and a
    /// receiver for `(request line, body)`.
    pub(crate) fn serve_once(
        status: u16,
        body: String,
    ) -> (String, mpsc::Receiver<(String, String)>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let mut len = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                let lower = line.to_ascii_lowercase();
                if let Some(v) = lower.strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap();
                }
            }
            let mut buf = vec![0u8; len];
            reader.read_exact(&mut buf).unwrap();
            let mut stream = stream;
            let reason = match status {
                200 => "OK",
                204 => "No Content",
                _ => "Status",
            };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            let _ = tx.send((
                request_line.trim().to_string(),
                String::from_utf8(buf).unwrap(),
            ));
        });
        (base, rx)
    }

    /// A 200 answer carrying `token_file` (an envelope) as the token object.
    pub(crate) fn token_answer(token_file: &str) -> String {
        let token: serde_json::Value = serde_json::from_str(token_file).unwrap();
        serde_json::json!({ "token": token }).to_string()
    }

    const TOKEN: &str = include_str!(
        "../../../../crates/valet-core/tests/fixtures/license/token-valid.valet-token"
    );
    const LICENSE_ID: &str = "6f1c2a3b-4d5e-4f60-8a71-92b3c4d5e6f7";
    const MACHINE: &str = "0123456789abcdef0123456789abcdef";

    fn local(base: &str) -> LicenseClient {
        LicenseClient::build(base, false, Duration::from_secs(5)).unwrap()
    }

    #[test]
    fn unconfigured_licensing_builds_no_client() {
        let before = CLIENTS_BUILT.with(|c| c.get());
        // The shipped files are empty in the repository.
        let shipped = LicenseClient::configured(
            valet_core::license::license_api_base(),
            valet_core::license::shipped_public_key(),
            true,
        );
        assert!(matches!(shipped, Err(ClientError::NotConfigured)));
        for (api, key) in [
            (None, None),
            (Some("https://license.example"), None),
            (None, Some("RWQ...")),
        ] {
            assert!(matches!(
                LicenseClient::configured(api, key, true),
                Err(ClientError::NotConfigured)
            ));
        }
        assert_eq!(CLIENTS_BUILT.with(|c| c.get()), before);
        assert_eq!(ClientError::NotConfigured.code(), "license.not_configured");
    }

    #[test]
    fn status_codes_map_to_contract_errors() {
        assert_eq!(status_error(404), ClientError::Unknown);
        assert_eq!(status_error(409), ClientError::TooMany);
        assert_eq!(status_error(410), ClientError::Revoked);
        for s in [400, 401, 403, 429, 500, 502, 503] {
            assert_eq!(status_error(s), ClientError::Network, "{s}");
        }
        assert!(ClientError::Unknown.drops_token());
        assert!(ClientError::TooMany.drops_token());
        assert!(ClientError::Revoked.drops_token());
        assert!(!ClientError::Network.drops_token());
        assert!(!ClientError::NotConfigured.drops_token());
    }

    #[test]
    fn production_client_refuses_plain_http() {
        let (base, _rx) = serve_once(200, token_answer(TOKEN));
        let client = LicenseClient::build(&base, true, Duration::from_secs(5)).unwrap();
        let r = tauri::async_runtime::block_on(client.validate(LICENSE_ID, MACHINE));
        assert_eq!(r, Err(ClientError::Network));
    }

    #[tokio::test]
    async fn validate_sends_only_the_three_fields_and_returns_the_token() {
        let (base, rx) = serve_once(200, token_answer(TOKEN));
        let token = local(&format!("{base}/"))
            .validate(LICENSE_ID, MACHINE)
            .await
            .unwrap();
        let (line, body) = rx.recv().unwrap();
        assert!(line.starts_with("POST /v1/validate "), "{line}");
        let body: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            body,
            serde_json::json!({
                "license_id": LICENSE_ID,
                "machine": MACHINE,
                "app_version": APP_VERSION,
            })
        );
        // Stored as the envelope JSON; it verifies with the TEST key.
        let key =
            include_str!("../../../../crates/valet-core/tests/fixtures/license/TEST-pubkey.txt");
        let t = valet_core::license::read_activation(&token, key).unwrap();
        assert_eq!(t.machine, MACHINE);
    }

    #[tokio::test]
    async fn activate_maps_refusals() {
        for (status, want) in [
            (404, ClientError::Unknown),
            (409, ClientError::TooMany),
            (410, ClientError::Revoked),
            (500, ClientError::Network),
            (503, ClientError::Network),
        ] {
            let (base, rx) = serve_once(status, "{}".into());
            let r = local(&base).activate(LICENSE_ID, MACHINE).await;
            assert_eq!(r, Err(want), "{status}");
            let (line, _) = rx.recv().unwrap();
            assert!(line.starts_with("POST /v1/activate "), "{line}");
        }
    }

    #[tokio::test]
    async fn malformed_200_answer_is_a_network_error() {
        for body in ["{}", "not json", r#"{"token":"a string"}"#] {
            let (base, _rx) = serve_once(200, body.into());
            let r = local(&base).validate(LICENSE_ID, MACHINE).await;
            assert_eq!(r, Err(ClientError::Network), "{body}");
        }
    }

    #[tokio::test]
    async fn deactivate_sends_license_and_machine_only() {
        let (base, rx) = serve_once(204, String::new());
        local(&base).deactivate(LICENSE_ID, MACHINE).await.unwrap();
        let (line, body) = rx.recv().unwrap();
        assert!(line.starts_with("POST /v1/deactivate "), "{line}");
        let body: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            body,
            serde_json::json!({ "license_id": LICENSE_ID, "machine": MACHINE })
        );
    }

    #[tokio::test]
    async fn no_server_and_timeout_are_network_errors() {
        // A port nobody listens on.
        let port = {
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let r = local(&format!("http://127.0.0.1:{port}"))
            .validate(LICENSE_ID, MACHINE)
            .await;
        assert_eq!(r, Err(ClientError::Network));

        // Accepts and never answers.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let hold = std::thread::spawn(move || {
            let conn = listener.accept();
            std::thread::sleep(Duration::from_millis(800));
            drop(conn);
        });
        let client = LicenseClient::build(&base, false, Duration::from_millis(300)).unwrap();
        assert_eq!(
            client.validate(LICENSE_ID, MACHINE).await,
            Err(ClientError::Network)
        );
        hold.join().unwrap();
    }
}
