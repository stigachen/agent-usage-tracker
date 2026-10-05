use super::*;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const SCOPE: &str = "https://auth.x.ai::b1a00492-073a-47ea-816f-4c329264a828";
const NO_HTTP: &str = "http://127.0.0.1:0";
const BILLING: &str = r#"{"config":{"creditUsagePercent":23,"currentPeriod":{"type":"WEEKLY","end":"2099-01-01T00:00:00Z"}}}"#;
const SETTINGS: &str = r#"{"subscription_tier_display":"SuperGrok"}"#;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "agent-usage-grok 用户 {}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&directory).unwrap();
        Self(directory.join("auth.json"))
    }

    fn write(&self, entry: Value) {
        write_auth(&self.0, entry);
    }

    async fn fetch(&self, endpoint: &str) -> UsageSnapshot {
        Grok.fetch_from_path(&client(), &self.0, None, endpoint, endpoint).await
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}

fn entry(email: &str, token: &str) -> Value {
    json!({ "key": token, "email": email, "user_id": email, "expires_at": "2099-01-01T00:00:00Z" })
}

fn write_auth(path: &Path, entry: Value) {
    std::fs::write(path, serde_json::to_vec(&json!({ SCOPE: entry })).unwrap()).unwrap();
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(5)).build().unwrap()
}

// A bounded local server, with credential mutations precisely between request and response.
// No real tokens, environment changes, CLI process, or remote API calls are used by these tests.
struct Server {
    endpoint: String,
    worker: JoinHandle<Vec<String>>,
}

impl Server {
    fn start(
        responses: Vec<(u16, Vec<u8>)>,
        mut on_request: impl FnMut(usize) + Send + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let worker = thread::spawn(move || {
            let mut requests = Vec::new();
            for (index, (status, body)) in responses.into_iter().enumerate() {
                let deadline = Instant::now() + Duration::from_secs(10);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "mock request {index} timed out");
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("mock accept: {error}"),
                    }
                };
                // Accepted sockets may inherit the listener's nonblocking mode on macOS/Windows.
                // Use blocking reads with a timeout so request arrival timing cannot cause WouldBlock.
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 1024];
                let header_end = loop {
                    let count = stream.read(&mut buffer).unwrap();
                    assert!(count > 0, "incomplete mock request");
                    bytes.extend_from_slice(&buffer[..count]);
                    if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        break end + 4;
                    }
                    assert!(bytes.len() < 16_384);
                };
                let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
                let content_length = headers.lines().find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>().unwrap())
                }).unwrap_or(0);
                while bytes.len() < header_end + content_length {
                    let count = stream.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buffer[..count]);
                }
                requests.push(headers);
                on_request(index);
                write!(stream, "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                stream.write_all(&body).unwrap();
            }
            requests
        });
        Self { endpoint, worker }
    }

    fn usage(on_request: impl FnMut(usize) + Send + 'static) -> Self {
        Self::start(vec![(200, BILLING.as_bytes().to_vec()), (200, SETTINGS.as_bytes().to_vec())], on_request)
    }

    fn finish(self, token: &str) -> Vec<String> {
        let requests = self.worker.join().unwrap();
        for request in &requests {
            assert!(request.to_ascii_lowercase().contains(&format!("authorization: bearer {token}\r\n")));
        }
        requests
    }
}

fn assert_no_usage(snapshot: &UsageSnapshot, issue: CredentialIssue) {
    assert_eq!(snapshot.credential_issue, Some(issue));
    assert!(snapshot.managed);
    assert!(snapshot.windows.is_empty());
    assert!(snapshot.plan.is_none());
    assert!(snapshot.period_ends_at.is_none());
}

#[tokio::test]
async fn reports_file_failures_without_exposing_contents_or_sending_requests() {
    let fixture = Fixture::new();
    let missing = fixture.fetch(NO_HTTP).await;
    assert_no_usage(&missing, CredentialIssue::Missing);
    assert!(missing.needs_auth);
    std::fs::create_dir(&fixture.0).unwrap();
    let unreadable = fixture.fetch(NO_HTTP).await;
    assert_no_usage(&unreadable, CredentialIssue::Unreadable);
    assert!(!unreadable.needs_auth);
    std::fs::remove_dir(&fixture.0).unwrap();
    std::fs::write(&fixture.0, br#"{"secret": "private-token", "broken": "#).unwrap();
    let invalid = fixture.fetch(NO_HTTP).await;
    assert_no_usage(&invalid, CredentialIssue::Invalid);
    assert!(!invalid.needs_auth);
    assert!(invalid.account_id.is_none());
    assert!(!serde_json::to_string(&invalid).unwrap().contains("private-token"));
    assert!(invalid.login_hint.unwrap().contains("invalid"));
}

#[tokio::test]
async fn does_not_send_expired_credentials_or_query_an_unexpected_account() {
    let fixture = Fixture::new();
    let mut expired = entry("a@example.com", "expired-token");
    expired["expires_at"] = json!("2000-01-01T00:00:00Z");
    fixture.write(expired);
    let snapshot = fixture.fetch(NO_HTTP).await;
    assert_no_usage(&snapshot, CredentialIssue::Expired);
    assert_eq!(snapshot.account_id.as_deref(), Some("a@example.com"));
    assert!(snapshot.needs_auth);

    fixture.write(entry("b@example.com", "new-token"));
    let snapshot = Grok.fetch_from_path(&client(), &fixture.0, Some("a@example.com"), NO_HTTP, NO_HTTP).await;
    assert_no_usage(&snapshot, CredentialIssue::Changed);
    assert_eq!(snapshot.account_id.as_deref(), Some("b@example.com"));
    assert!(!snapshot.needs_auth);
}

#[tokio::test]
async fn fetches_usage_read_only_with_one_captured_bearer() {
    let fixture = Fixture::new();
    fixture.write(entry("a@example.com", "original-token"));
    let before = std::fs::read(&fixture.0).unwrap();
    let server = Server::usage(|_| {});
    let snapshot = fixture.fetch(&server.endpoint).await;
    assert_eq!(snapshot.credential_issue, None);
    assert_eq!(snapshot.account_id.as_deref(), Some("a@example.com"));
    assert_eq!(snapshot.plan.as_deref(), Some("SuperGrok"));
    assert_eq!(snapshot.windows[0].used, 23.0);
    assert_eq!(snapshot.windows[0].label, "Weekly limit");
    assert_eq!(std::fs::read(&fixture.0).unwrap(), before);
    assert!(!serde_json::to_string(&snapshot).unwrap().contains("original-token"));
    let requests = server.finish("original-token");
    assert!(requests[0].starts_with("GET /billing?format=credits "));
    assert!(requests[1].starts_with("GET /settings "));
    for request in requests {
        assert!(request.to_ascii_lowercase().contains("x-xai-token-auth: xai-grok-cli\r\n"));
    }
}

#[tokio::test]
async fn discards_usage_if_account_or_principal_changes_during_requests() {
    for field in ["email", "user_id", "principal_type", "principal_id", "team_id"] {
        let fixture = Fixture::new();
        fixture.write(entry("a@example.com", "original-token"));
        let path = fixture.0.clone();
        let server = Server::usage(move |index| {
            if index == 0 {
                let mut changed = entry("a@example.com", "replacement-token");
                changed[field] = json!("other");
                write_auth(&path, changed);
            }
        });
        let snapshot = fixture.fetch(&server.endpoint).await;
        assert_no_usage(&snapshot, CredentialIssue::Changed);
        assert!(!snapshot.needs_auth);
        assert_eq!(snapshot.account_id.as_deref(), Some(if field == "email" { "other" } else { "a@example.com" }));
        server.finish("original-token");
    }
}

#[tokio::test]
async fn preserves_success_during_same_account_token_rotation() {
    let fixture = Fixture::new();
    fixture.write(entry("a@example.com", "original-token"));
    let path = fixture.0.clone();
    let server = Server::usage(move |index| {
        if index == 0 {
            write_auth(&path, entry("a@example.com", "replacement-token"));
        }
    });
    let snapshot = fixture.fetch(&server.endpoint).await;
    assert_eq!(snapshot.credential_issue, None);
    assert_eq!(snapshot.windows[0].used, 23.0);
    server.finish("original-token");
}

#[tokio::test]
async fn does_not_apply_old_token_rejection_to_its_replacement() {
    for rotate in [false, true] {
        let fixture = Fixture::new();
        fixture.write(entry("a@example.com", "original-token"));
        let path = fixture.0.clone();
        let server = Server::start(vec![(401, b"{}".to_vec())], move |_| {
            if rotate {
                write_auth(&path, entry("a@example.com", "replacement-token"));
            }
        });
        let snapshot = fixture.fetch(&server.endpoint).await;
        assert_no_usage(&snapshot, if rotate { CredentialIssue::Changed } else { CredentialIssue::Rejected });
        assert_eq!(snapshot.needs_auth, !rotate);
        server.finish("original-token");
    }
}

#[tokio::test]
async fn discards_usage_if_credentials_disappear_or_become_invalid() {
    for remove in [false, true] {
        let fixture = Fixture::new();
        fixture.write(entry("a@example.com", "original-token"));
        let path = fixture.0.clone();
        let server = Server::usage(move |index| {
            if index == 1 {
                if remove { std::fs::remove_file(&path).unwrap(); }
                else { std::fs::write(&path, b"{").unwrap(); }
            }
        });
        let snapshot = fixture.fetch(&server.endpoint).await;
        assert_no_usage(&snapshot, if remove { CredentialIssue::Missing } else { CredentialIssue::Invalid });
        assert!(snapshot.account_id.is_none());
        server.finish("original-token");
    }
}

#[tokio::test]
async fn grpc_fallback_uses_the_same_bearer_and_rechecks_identity_afterwards() {
    for switch_account in [false, true] {
        let fixture = Fixture::new();
        fixture.write(entry("a@example.com", "original-token"));
        // gRPC-web frame containing config { credit_usage_percent: 42.0 }.
        let mut grpc = vec![0, 0, 0, 0, 7, 10, 5, 13];
        grpc.extend_from_slice(&42.0_f32.to_le_bytes());
        let path = fixture.0.clone();
        let server = Server::start(vec![
            (200, br#"{"config":{}}"#.to_vec()),
            (200, SETTINGS.as_bytes().to_vec()),
            (200, grpc),
        ], move |index| {
            if switch_account && index == 2 {
                write_auth(&path, entry("b@example.com", "replacement-token"));
            }
        });
        let snapshot = fixture.fetch(&server.endpoint).await;
        if switch_account {
            assert_no_usage(&snapshot, CredentialIssue::Changed);
            assert_eq!(snapshot.account_id.as_deref(), Some("b@example.com"));
        } else {
            assert_eq!(snapshot.credential_issue, None);
            assert_eq!(snapshot.windows[0].used, 42.0);
            assert_eq!(snapshot.account_id.as_deref(), Some("a@example.com"));
        }
        let requests = server.finish("original-token");
        assert!(requests[2].starts_with("POST / "));
    }
}
