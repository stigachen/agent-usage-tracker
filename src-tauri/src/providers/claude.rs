//! Claude Code (Claude.ai plan) usage. Credentials are owned and refreshed by Claude Code:
//! the macOS Keychain item "Claude Code-credentials", or `~/.claude/.credentials.json`
//! elsewhere. We only read them, never write.

use super::{Credential, DeviceCode, Provider, UsageSnapshot, UsageWindow};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;

pub struct Claude;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredFile {
    claude_ai_oauth: Option<OAuth>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OAuth {
    access_token: String,
    subscription_type: Option<String>,
}

#[derive(Deserialize)]
struct UsageResp {
    five_hour: Option<Window>,
    seven_day: Option<Window>,
    seven_day_opus: Option<Window>,
    seven_day_sonnet: Option<Window>,
}

#[derive(Deserialize)]
struct Window {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

fn config_dir() -> Option<PathBuf> {
    match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(d) => Some(PathBuf::from(d)),
        None => dirs::home_dir().map(|h| h.join(".claude")),
    }
}

fn read_cred_json() -> Option<Vec<u8>> {
    if let Some(b) = config_dir().and_then(|d| std::fs::read(d.join(".credentials.json")).ok()) {
        return Some(b);
    }
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("security")
            .args(["find-generic-password", "-s", "Claude Code-credentials", "-w"])
            .output()
            .ok()?;
        if out.status.success() {
            return Some(out.stdout);
        }
    }
    None
}

fn read_oauth() -> Option<OAuth> {
    serde_json::from_slice::<CredFile>(&read_cred_json()?).ok()?.claude_ai_oauth
}

/// Signed-in email from `~/.claude.json`; used as the account id.
fn email() -> Option<String> {
    let path = match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(d) => PathBuf::from(d).join(".claude.json"),
        None => dirs::home_dir()?.join(".claude.json"),
    };
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    v.get("oauthAccount")?.get("emailAddress")?.as_str().map(Into::into)
}

#[async_trait]
impl Provider for Claude {
    fn id(&self) -> &'static str {
        "claude"
    }
    fn name(&self) -> &'static str {
        "Claude Code"
    }
    fn login_hint(&self) -> Option<&'static str> {
        Some("Run `claude` and sign in with /login, then refresh.")
    }

    fn discover(&self) -> Vec<Credential> {
        read_oauth()
            .map(|_| Credential { account_id: email().unwrap_or_else(|| "Claude Code".into()), secret: String::new() })
            .into_iter()
            .collect()
    }

    async fn fetch(&self, http: &reqwest::Client, account_id: &str, _secret: &str) -> UsageSnapshot {
        let mut snap = UsageSnapshot::empty(self, Some(account_id));
        snap.managed = true;
        // Re-read on every fetch: Claude Code rotates tokens in place.
        let Some(t) = read_oauth() else {
            snap.needs_auth = true;
            return snap;
        };
        snap.plan = t.subscription_type;
        let req = http
            .get("https://api.anthropic.com/api/oauth/usage")
            .bearer_auth(&t.access_token)
            .header("anthropic-beta", "oauth-2025-04-20")
            .header("User-Agent", "claude-code");
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
        snap.windows = [
            ("5-hour limit", body.five_hour),
            ("Weekly limit", body.seven_day),
            ("Weekly Opus limit", body.seven_day_opus),
            ("Weekly Sonnet limit", body.seven_day_sonnet),
        ]
        .into_iter()
        .filter_map(|(label, w)| {
            let w = w?;
            Some(UsageWindow { label: label.into(), used: w.utilization?, limit: Some(100.0), resets_at: w.resets_at })
        })
        .collect();
        snap
    }

    async fn start_login(&self, _http: &reqwest::Client) -> Result<DeviceCode, String> {
        Err("Claude Code accounts come from Claude Code. Run `claude` and /login.".into())
    }

    async fn finish_login(&self, _http: &reqwest::Client, _code: DeviceCode) -> Result<Credential, String> {
        Err("unsupported".into())
    }
}
