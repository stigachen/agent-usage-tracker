//! Codex (ChatGPT plan) usage. Credentials are owned and refreshed by the Codex CLI
//! in `~/.codex/auth.json`; we only read them, never write.

use super::{Credential, DeviceCode, Provider, UsageSnapshot, UsageWindow};
use async_trait::async_trait;
use base64::Engine;
use serde::Deserialize;
use std::path::PathBuf;

pub struct Codex;

#[derive(Deserialize)]
struct AuthFile {
    tokens: Option<Tokens>,
}

#[derive(Deserialize)]
struct Tokens {
    id_token: String,
    access_token: String,
    account_id: Option<String>,
}

#[derive(Deserialize)]
struct UsageResp {
    plan_type: Option<String>,
    rate_limit: Option<RateLimit>,
}

#[derive(Deserialize)]
struct RateLimit {
    primary_window: Option<Window>,
    secondary_window: Option<Window>,
}

#[derive(Deserialize)]
struct Window {
    used_percent: f64,
    limit_window_seconds: Option<u64>,
    reset_at: Option<i64>,
}

fn auth_path() -> Option<PathBuf> {
    match std::env::var_os("CODEX_HOME") {
        Some(h) => Some(PathBuf::from(h).join("auth.json")),
        None => dirs::home_dir().map(|h| h.join(".codex/auth.json")),
    }
}

fn read_tokens() -> Option<Tokens> {
    let bytes = std::fs::read(auth_path()?).ok()?;
    serde_json::from_slice::<AuthFile>(&bytes).ok()?.tokens
}

/// Email claim from the id_token; used as the account id.
fn email(id_token: &str) -> Option<String> {
    let payload = id_token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()?
        .get("email")?
        .as_str()
        .map(Into::into)
}

fn window_label(secs: Option<u64>) -> String {
    match secs {
        Some(s) if s <= 6 * 3600 => format!("{}-hour limit", (s + 1800) / 3600),
        Some(s) if (6 * 86400..=8 * 86400).contains(&s) => "Weekly limit".into(),
        Some(s) => format!("{}-day limit", (s + 43200) / 86400),
        None => "Usage limit".into(),
    }
}

#[async_trait]
impl Provider for Codex {
    fn id(&self) -> &'static str {
        "codex"
    }
    fn name(&self) -> &'static str {
        "Codex"
    }
    fn login_hint(&self) -> Option<&'static str> {
        Some("Run `codex login` in a terminal, then refresh.")
    }

    fn discover(&self) -> Vec<Credential> {
        read_tokens()
            .and_then(|t| email(&t.id_token))
            .map(|account_id| Credential { account_id, secret: String::new() })
            .into_iter()
            .collect()
    }

    async fn fetch(&self, http: &reqwest::Client, account_id: &str, _secret: &str) -> UsageSnapshot {
        let mut snap = UsageSnapshot::empty(self, Some(account_id));
        snap.managed = true;
        // Re-read on every fetch: the CLI rotates tokens in place.
        let Some(t) = read_tokens() else {
            snap.needs_auth = true;
            return snap;
        };
        let mut req = http
            .get("https://chatgpt.com/backend-api/wham/usage")
            .bearer_auth(&t.access_token)
            .header("User-Agent", "codex-cli");
        if let Some(id) = &t.account_id {
            req = req.header("ChatGPT-Account-Id", id);
        }
        let body: UsageResp = match req.send().await {
            Ok(r) if r.status() == 401 || r.status() == 403 => {
                snap.needs_auth = true;
                return snap;
            }
            Ok(r) => match r.error_for_status() {
                Ok(r) => match r.json().await {
                    Ok(b) => b,
                    Err(e) => {
                        snap.error = Some(format!("Bad response: {e}"));
                        return snap;
                    }
                },
                Err(e) => {
                    snap.error = Some(e.to_string());
                    return snap;
                }
            },
            Err(e) => {
                snap.error = Some(e.to_string());
                return snap;
            }
        };
        snap.plan = body.plan_type;
        let rl = body.rate_limit;
        snap.windows = rl
            .into_iter()
            .flat_map(|r| [r.primary_window, r.secondary_window])
            .flatten()
            .map(|w| UsageWindow {
                label: window_label(w.limit_window_seconds),
                used: w.used_percent,
                limit: Some(100.0),
                resets_at: w
                    .reset_at
                    .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
                    .map(|d| d.to_rfc3339()),
            })
            .collect();
        snap
    }

    async fn start_login(&self, _http: &reqwest::Client) -> Result<DeviceCode, String> {
        Err("Codex accounts come from the Codex CLI. Run `codex login`.".into())
    }

    async fn finish_login(&self, _http: &reqwest::Client, _code: DeviceCode) -> Result<Credential, String> {
        Err("unsupported".into())
    }
}
