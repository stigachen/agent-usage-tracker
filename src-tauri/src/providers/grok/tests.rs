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

struct Fixture(PathBuf, Grok);

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
        Self(directory.join("auth.json"), Grok::default())
    }

    fn write(&self, entry: Value) {
        write_auth(&self.0, entry);
    }

    async fn fetch(&self, endpoint: &str) -> UsageSnapshot {
        self.1.fetch_from_path(&client(), &self.0, None, endpoint, endpoint).await
    }

    async fn seed(&self) -> UsageSnapshot {
        self.write(entry("a@example.com", "original-token"));
        let server = Server::usage(|_| {});
        let snapshot = self.fetch(&server.endpoint).await;
        server.finish("original-token");
        assert_eq!(snapshot.windows[0].used, 23.0);
        snapshot
    }

    fn poll(&self) -> bool {
        let stamp = monitor::Stamp::read(Ok(self.0.clone()), chrono::Utc::now());
        self.1.monitor.lock().unwrap().observe(stamp)
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
    let snapshot = fixture.1.fetch_from_path(&client(), &fixture.0, Some("a@example.com"), NO_HTTP, NO_HTTP).await;
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

#[tokio::test]
async fn retains_last_success_through_expiry_and_clears_staleness_on_recovery() {
    let fixture = Fixture::new();
    let success = fixture.seed().await;
    assert_eq!(success.last_success_at, Some(success.fetched_at.clone()));
    assert!(!success.stale);

    let mut expired = entry("a@example.com", "original-token");
    expired["expires_at"] = json!("2000-01-01T00:00:00Z");
    fixture.write(expired);
    for _ in 0..2 {
        let failed = fixture.fetch(NO_HTTP).await;
        assert_eq!(failed.credential_issue, Some(CredentialIssue::Expired));
        assert!(failed.stale);
        assert!(failed.needs_auth);
        assert_eq!(failed.windows[0].used, 23.0);
        assert_eq!(failed.last_success_at, success.last_success_at);
        assert_eq!(failed.min_remaining(), None);
    }

    fixture.write(entry("a@example.com", "renewed-token"));
    let server = Server::usage(|_| {});
    let recovered = fixture.fetch(&server.endpoint).await;
    server.finish("renewed-token");
    assert!(!recovered.stale);
    assert!(!recovered.needs_auth);
    assert_eq!(recovered.credential_issue, None);
    assert_eq!(recovered.last_success_at, Some(recovered.fetched_at.clone()));
    assert!(recovered.min_remaining().is_some());
}

#[tokio::test]
async fn distinguishes_network_service_response_and_rejected_sign_in_with_history() {
    let fixture = Fixture::new();
    let success = fixture.seed().await;
    let offline = fixture.fetch(NO_HTTP).await;
    assert_eq!(offline.fetch_issue, Some(FetchIssue::Network));
    assert!(!offline.needs_auth);
    assert!(offline.stale);
    assert_eq!(offline.last_success_at, success.last_success_at);
    for (status, body, fetch_issue, credential_issue) in [
        (503, "{}", Some(FetchIssue::Service), None),
        (200, "{", Some(FetchIssue::Response), None),
        (401, "{}", None, Some(CredentialIssue::Rejected)),
        (403, "{}", None, Some(CredentialIssue::Rejected)),
    ] {
        let server = Server::start(vec![(status, body.as_bytes().to_vec())], |_| {});
        let failed = fixture.fetch(&server.endpoint).await;
        server.finish("original-token");
        assert_eq!(failed.fetch_issue, fetch_issue);
        assert_eq!(failed.credential_issue, credential_issue);
        assert_eq!(failed.needs_auth, credential_issue.is_some());
        assert!(failed.stale);
        assert_eq!(failed.windows[0].used, 23.0);
        assert_eq!(failed.last_success_at, success.last_success_at);
        assert_eq!(failed.min_remaining(), None);
    }
}

#[tokio::test]
async fn clears_history_across_refreshes_even_if_the_new_principal_has_the_same_email() {
    for field in ["email", "user_id", "principal_type", "principal_id", "team_id"] {
        let fixture = Fixture::new();
        fixture.seed().await;
        let mut replacement = entry("a@example.com", "replacement-token");
        replacement[field] = json!("other");
        fixture.write(replacement);
        let failed = fixture.fetch(NO_HTTP).await;
        assert!(!failed.stale, "cached a different {field}");
        assert!(failed.windows.is_empty());
        assert!(failed.last_success_at.is_none());
        assert!(failed.plan.is_none());
        // Switching back must not revive an old account's history either.
        fixture.write(entry("a@example.com", "original-token"));
        assert!(fixture.fetch(NO_HTTP).await.windows.is_empty());
    }
}

#[tokio::test]
async fn unknown_identity_discards_history_and_does_not_restore_it_from_a_later_read() {
    for failure in ["missing", "invalid", "unreadable", "unsupported", "ambiguous"] {
        let fixture = Fixture::new();
        fixture.seed().await;
        match failure {
            "missing" => std::fs::remove_file(&fixture.0).unwrap(),
            "invalid" => std::fs::write(&fixture.0, b"{").unwrap(),
            "unreadable" => {
                std::fs::remove_file(&fixture.0).unwrap();
                std::fs::create_dir(&fixture.0).unwrap();
            }
            "unsupported" => std::fs::write(&fixture.0, br#"{"other":{}}"#).unwrap(),
            _ => std::fs::write(&fixture.0, br#"{"https://auth.x.ai::a":{},"https://auth.x.ai::b":{}}"#).unwrap(),
        }
        let failed = fixture.fetch(NO_HTTP).await;
        assert!(failed.credential_issue.is_some());
        assert!(!failed.stale);
        assert!(failed.windows.is_empty());
        assert!(failed.account_id.is_none());
        assert!(failed.last_success_at.is_none());
        if failure == "unreadable" { std::fs::remove_dir(&fixture.0).unwrap(); }
        fixture.write(entry("a@example.com", "original-token"));
        assert!(fixture.fetch(NO_HTTP).await.windows.is_empty());
    }
}

#[tokio::test]
async fn different_directory_or_scope_cannot_reuse_history() {
    let fixture = Fixture::new();
    fixture.seed().await;
    let other = Fixture::new();
    other.write(entry("a@example.com", "original-token"));
    let snapshot = fixture.1.fetch_from_path(&client(), &other.0, None, NO_HTTP, NO_HTTP).await;
    assert!(snapshot.windows.is_empty());
    assert!(!snapshot.stale);

    fixture.seed().await;
    std::fs::write(&fixture.0, serde_json::to_vec(&json!({
        "https://accounts.x.ai/sign-in": entry("a@example.com", "original-token"),
    })).unwrap()).unwrap();
    assert!(fixture.fetch(NO_HTTP).await.windows.is_empty());
}

#[tokio::test]
async fn grpc_failure_preserves_history_but_a_successful_no_quota_response_supersedes_it() {
    let fixture = Fixture::new();
    let success = fixture.seed().await;
    for (status, body, issue) in [(503, b"{}".to_vec(), FetchIssue::Service), (200, b"invalid frame".to_vec(), FetchIssue::Response)] {
        let server = Server::start(vec![
            (200, br#"{"config":{}}"#.to_vec()), (200, SETTINGS.as_bytes().to_vec()), (status, body),
        ], |_| {});
        let failed = fixture.fetch(&server.endpoint).await;
        server.finish("original-token");
        assert_eq!(failed.fetch_issue, Some(issue));
        assert!(failed.stale);
        assert_eq!(failed.last_success_at, success.last_success_at);
    }
    let server = Server::start(vec![(200, b"{}".to_vec()), (200, SETTINGS.as_bytes().to_vec())], |_| {});
    let empty = fixture.fetch(&server.endpoint).await;
    server.finish("original-token");
    assert!(!empty.stale);
    assert!(empty.note.is_some());
    assert!(empty.windows.is_empty());
    assert!(fixture.fetch(NO_HTTP).await.windows.is_empty());
}

#[tokio::test]
async fn concurrent_refreshes_are_serialized_and_preserve_the_newest_success() {
    let fixture = Fixture::new();
    fixture.seed().await;
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let mut started_tx = Some(started_tx);
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let slow = Server::start(vec![(503, b"{}".to_vec())], move |_| {
        started_tx.take().unwrap().send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    let requested = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let requested_in_server = requested.clone();
    let fast = Server::start(vec![
        (200, BILLING.replace("23", "44").into_bytes()), (200, SETTINGS.as_bytes().to_vec()),
    ], move |_| { requested_in_server.store(true, Ordering::SeqCst); });
    let second = async {
        started_rx.await.unwrap();
        let release = async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            assert!(!requested.load(Ordering::SeqCst), "overlapping HTTP requests");
            release_tx.send(()).unwrap();
        };
        let (snapshot, ()) = tokio::join!(fixture.fetch(&fast.endpoint), release);
        snapshot
    };
    let (failed, recovered) = tokio::join!(fixture.fetch(&slow.endpoint), second);
    slow.finish("original-token");
    fast.finish("original-token");
    assert!(failed.stale);
    assert_eq!(failed.windows[0].used, 23.0);
    assert!(!recovered.stale);
    assert_eq!(recovered.windows[0].used, 44.0);
    let next_failure = fixture.fetch(NO_HTTP).await;
    assert_eq!(next_failure.windows[0].used, 44.0);
    assert_eq!(next_failure.last_success_at, recovered.last_success_at);
}

#[test]
fn monitor_debounces_creation_replacement_and_deletion_and_ignores_unchanged_rewrites() {
    let fixture = Fixture::new();
    assert!(!fixture.poll()); // a missing file is a valid baseline
    fixture.write(entry("a@example.com", "original-token"));
    assert!(!fixture.poll());
    assert!(fixture.poll());
    for _ in 0..3 {
        fixture.write(entry("a@example.com", "original-token"));
        assert!(!fixture.poll());
    }

    std::fs::write(&fixture.0, b"{").unwrap();
    assert!(!fixture.poll());
    let replacement = fixture.0.with_extension("replacement");
    write_auth(&replacement, entry("a@example.com", "renewed-token"));
    std::fs::rename(&replacement, &fixture.0).unwrap();
    assert!(!fixture.poll());
    assert!(fixture.poll());
    assert!(!fixture.poll());

    std::fs::remove_file(&fixture.0).unwrap();
    std::fs::remove_dir(fixture.0.parent().unwrap()).unwrap();
    assert!(!fixture.poll());
    assert!(fixture.poll());
    assert!(!fixture.poll());
    std::fs::create_dir(fixture.0.parent().unwrap()).unwrap();
    fixture.write(entry("a@example.com", "renewed-token"));
    assert!(!fixture.poll());
    assert!(fixture.poll());
}

#[test]
fn monitor_detects_expiry_without_a_file_change_and_ignores_unselected_records() {
    let fixture = Fixture::new();
    fixture.write(entry("a@example.com", "original-token"));
    assert!(!fixture.poll());
    std::fs::write(&fixture.0, serde_json::to_vec(&json!({
        SCOPE: entry("a@example.com", "original-token"), "unrelated": { "key": 42 },
    })).unwrap()).unwrap();
    assert!(!fixture.poll());
    assert!(!fixture.poll());
    let expired_at = chrono::DateTime::parse_from_rfc3339("2099-01-01T00:00:00Z").unwrap().with_timezone(&chrono::Utc);
    let stamp = monitor::Stamp::read(Ok(fixture.0.clone()), expired_at);
    let mut monitor = fixture.1.monitor.lock().unwrap();
    assert!(!monitor.observe(stamp.clone()));
    assert!(monitor.observe(stamp.clone()));
    assert!(!monitor.observe(stamp));
}

#[tokio::test]
async fn a_change_during_a_request_is_detected_afterwards_and_the_new_account_recovers() {
    let fixture = Fixture::new();
    fixture.seed().await;
    assert!(!fixture.poll());
    let path = fixture.0.clone();
    let server = Server::usage(move |index| {
        if index == 0 { write_auth(&path, entry("b@example.com", "replacement-token")); }
    });
    let changed = fixture.fetch(&server.endpoint).await;
    server.finish("original-token");
    assert_no_usage(&changed, CredentialIssue::Changed);
    assert!(!fixture.poll());
    assert!(fixture.poll());
    let next = Server::usage(|_| {});
    let recovered = fixture.fetch(&next.endpoint).await;
    next.finish("replacement-token");
    assert_eq!(recovered.account_id.as_deref(), Some("b@example.com"));
    assert!(!recovered.stale);
    assert_eq!(recovered.credential_issue, None);
    assert_eq!(recovered.windows[0].used, 23.0);
    assert!(!fixture.poll());
}

#[tokio::test]
async fn transient_read_failure_recovers_even_when_the_same_credentials_return_between_polls() {
    for during_request in [false, true] {
        let fixture = Fixture::new();
        fixture.seed().await;
        assert!(!fixture.poll());
        let failed = if during_request {
            let path = fixture.0.clone();
            let server = Server::usage(move |index| {
                if index == 1 { std::fs::remove_file(&path).unwrap(); }
            });
            let snapshot = fixture.fetch(&server.endpoint).await;
            server.finish("original-token");
            snapshot
        } else {
            std::fs::remove_file(&fixture.0).unwrap();
            fixture.fetch(NO_HTTP).await
        };
        assert_no_usage(&failed, CredentialIssue::Missing);
        // No monitor poll saw the missing file. Its original contents return unchanged.
        fixture.write(entry("a@example.com", "original-token"));
        assert!(!fixture.poll());
        assert!(fixture.poll());
        let server = Server::usage(|_| {});
        let recovered = fixture.fetch(&server.endpoint).await;
        server.finish("original-token");
        assert!(!recovered.stale);
        assert_eq!(recovered.credential_issue, None);
        assert_eq!(recovered.windows[0].used, 23.0);
        assert!(!fixture.poll());
    }
}
