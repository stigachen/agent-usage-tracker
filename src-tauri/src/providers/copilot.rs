use super::{keyring_entry, DeviceCode, Provider, UsageSnapshot, UsageWindow};
use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

// Public OAuth client id used by the Copilot editor plugins.
const CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const UA: &str = "GitHubCopilotChat/0.26.0";

pub struct Copilot;

#[derive(Deserialize)]
struct UserResp {
    login: Option<String>,
    copilot_plan: Option<String>,
    quota_reset_date_utc: Option<String>,
    quota_snapshots: Option<std::collections::HashMap<String, Quota>>,
}

#[derive(Deserialize)]
struct Quota {
    unlimited: bool,
    entitlement: f64,
    remaining: f64,
}

#[derive(Deserialize)]
struct TokenResp {
    access_token: Option<String>,
    error: Option<String>,
    interval: Option<u64>,
}

fn label(id: &str) -> String {
    match id {
        "premium_interactions" => "Premium requests".into(),
        "chat" => "Chat".into(),
        "completions" => "Completions".into(),
        other => other.replace('_', " "),
    }
}

fn token() -> Option<String> {
    keyring_entry("copilot").ok()?.get_password().ok()
}

#[async_trait]
impl Provider for Copilot {
    fn id(&self) -> &'static str {
        "copilot"
    }
    fn name(&self) -> &'static str {
        "GitHub Copilot"
    }

    async fn fetch(&self, http: &reqwest::Client) -> UsageSnapshot {
        let mut snap = UsageSnapshot::empty(self);
        let Some(tok) = token() else {
            snap.needs_auth = true;
            return snap;
        };
        let res = http
            .get("https://api.github.com/copilot_internal/user")
            .header("Authorization", format!("token {tok}"))
            .header("Editor-Version", "vscode/1.99.0")
            .header("User-Agent", UA)
            .send()
            .await;
        let resp = match res {
            Ok(r) if r.status() == 401 => {
                snap.needs_auth = true;
                return snap;
            }
            Ok(r) => r,
            Err(e) => {
                snap.error = Some(e.to_string());
                return snap;
            }
        };
        let body: UserResp = match resp.error_for_status() {
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
        };
        snap.account = body.login;
        snap.plan = body.copilot_plan;
        let mut quotas: Vec<_> = body.quota_snapshots.unwrap_or_default().into_iter().collect();
        // Limited quotas first, premium on top.
        quotas.sort_by_key(|(id, q)| (q.unlimited, id != "premium_interactions", id.clone()));
        snap.windows = quotas
            .into_iter()
            .map(|(id, q)| UsageWindow {
                label: label(&id),
                used: if q.unlimited { 0.0 } else { (q.entitlement - q.remaining).max(0.0) },
                limit: (!q.unlimited).then_some(q.entitlement),
                resets_at: body.quota_reset_date_utc.clone(),
            })
            .collect();
        snap
    }

    async fn start_login(&self, http: &reqwest::Client) -> Result<DeviceCode, String> {
        #[derive(Deserialize)]
        struct R {
            device_code: String,
            user_code: String,
            verification_uri: String,
            interval: u64,
        }
        let r: R = http
            .post("https://github.com/login/device/code")
            .header("Accept", "application/json")
            .header("User-Agent", UA)
            .form(&[("client_id", CLIENT_ID), ("scope", "read:user")])
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        Ok(DeviceCode {
            user_code: r.user_code,
            verification_uri: r.verification_uri,
            device_code: r.device_code,
            interval: r.interval,
        })
    }

    async fn finish_login(&self, http: &reqwest::Client, code: DeviceCode) -> Result<(), String> {
        let mut interval = code.interval.max(5);
        for _ in 0..180 {
            tokio::time::sleep(Duration::from_secs(interval)).await;
            let r: TokenResp = http
                .post("https://github.com/login/oauth/access_token")
                .header("Accept", "application/json")
                .header("User-Agent", UA)
                .form(&[
                    ("client_id", CLIENT_ID),
                    ("device_code", code.device_code.as_str()),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await
                .map_err(|e| e.to_string())?
                .json()
                .await
                .map_err(|e| e.to_string())?;
            if let Some(tok) = r.access_token {
                return keyring_entry("copilot")
                    .and_then(|e| e.set_password(&tok))
                    .map_err(|e| e.to_string());
            }
            match r.error.as_deref() {
                Some("authorization_pending") => {}
                Some("slow_down") => interval = r.interval.unwrap_or(interval + 5),
                Some(e) => return Err(e.to_string()),
                None => return Err("unexpected response".into()),
            }
        }
        Err("login timed out".into())
    }

    fn logout(&self) -> Result<(), String> {
        match keyring_entry("copilot").and_then(|e| e.delete_credential()) {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}
