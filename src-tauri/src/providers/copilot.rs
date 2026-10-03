use super::{Billing, Credential, DeviceCode, ModelUsage, Provider, UsageSnapshot, UsageWindow};
use crate::store;
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

#[derive(Deserialize)]
struct GhUser {
    login: String,
}

/// Keychain namespace for the optional personal access token used for billing reports.
pub const BILLING_KEY: &str = "copilot-billing";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BillingResp {
    #[serde(default)]
    usage_items: Vec<BillingItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[derive(Default)]
struct BillingItem {
    model: String,
    discount_quantity: f64,
    discount_amount: f64,
    net_quantity: f64,
    net_amount: f64,
}

fn gh_get(http: &reqwest::Client, url: &str, pat: &str) -> reqwest::RequestBuilder {
    http.get(url)
        .header("Authorization", format!("Bearer {pat}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", UA)
}

/// Current month's AI credit usage per model for `login`.
async fn billing_report(http: &reqwest::Client, login: &str, pat: &str) -> Result<Billing, String> {
    use chrono::Datelike;
    let now = chrono::Utc::now();
    let url = format!(
        "https://api.github.com/users/{login}/settings/billing/ai_credit/usage?year={}&month={}",
        now.year(),
        now.month()
    );
    let r = gh_get(http, &url, pat).send().await.map_err(|e| e.to_string())?;
    match r.status().as_u16() {
        200 => {}
        401 => return Err("Billing token is invalid or expired".into()),
        403 | 404 => return Err("Billing token lacks the \"user\" scope".into()),
        s => return Err(format!("Billing request failed (HTTP {s})")),
    }
    let body: BillingResp = r.json().await.map_err(|e| format!("Bad billing response: {e}"))?;
    let mut models: Vec<ModelUsage> = vec![];
    for it in body.usage_items {
        let name = if it.model.is_empty() { "Other".to_string() } else { it.model };
        let m = match models.iter_mut().find(|m| m.model == name) {
            Some(m) => m,
            None => {
                models.push(ModelUsage {
                    model: name,
                    included: 0.0,
                    included_amount: 0.0,
                    additional: 0.0,
                    additional_amount: 0.0,
                });
                models.last_mut().unwrap()
            }
        };
        m.included += it.discount_quantity;
        m.included_amount += it.discount_amount;
        m.additional += it.net_quantity;
        m.additional_amount += it.net_amount;
    }
    models.retain(|m| m.included + m.additional > 0.0);
    models.sort_by(|a, b| (b.included + b.additional).total_cmp(&(a.included + a.additional)));
    let additional_amount = models.iter().map(|m| m.additional_amount).sum();
    Ok(Billing { models, additional_amount, error: None })
}

/// Checks that `pat` belongs to `login` and can read its billing report.
pub async fn verify_billing_token(http: &reqwest::Client, login: &str, pat: &str) -> Result<(), String> {
    let r = gh_get(http, "https://api.github.com/user", pat).send().await.map_err(|e| e.to_string())?;
    if r.status() == 401 {
        return Err("Token is invalid or expired".into());
    }
    let user: GhUser = r.error_for_status().map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    if !user.login.eq_ignore_ascii_case(login) {
        return Err(format!("This token belongs to @{}, not @{login}", user.login));
    }
    billing_report(http, login, pat).await.map(|_| ())
}

#[async_trait]
impl Provider for Copilot {
    fn id(&self) -> &'static str {
        "copilot"
    }
    fn name(&self) -> &'static str {
        "GitHub Copilot"
    }

    async fn fetch(&self, http: &reqwest::Client, account_id: &str, tok: &str) -> UsageSnapshot {
        let mut snap = UsageSnapshot::empty(self, Some(account_id));
        let billing_pat = store::get_secret(BILLING_KEY, account_id);
        snap.billing_configured = billing_pat.is_some();
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
        snap.account = body.login.or(snap.account);
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
        if let Some(pat) = billing_pat {
            snap.billing = Some(billing_report(http, account_id, &pat).await.unwrap_or_else(|e| Billing {
                models: vec![],
                additional_amount: 0.0,
                error: Some(e),
            }));
        }
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

    async fn finish_login(&self, http: &reqwest::Client, code: DeviceCode) -> Result<Credential, String> {
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
                let user: GhUser = http
                    .get("https://api.github.com/user")
                    .header("Authorization", format!("token {tok}"))
                    .header("User-Agent", UA)
                    .send()
                    .await
                    .and_then(|r| r.error_for_status())
                    .map_err(|e| e.to_string())?
                    .json()
                    .await
                    .map_err(|e| e.to_string())?;
                return Ok(Credential { account_id: user.login, secret: tok });
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
}
