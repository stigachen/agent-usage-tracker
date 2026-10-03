pub mod claude;
pub mod codex;
pub mod copilot;
pub mod grok;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// One quota window, e.g. "Premium requests" for the current month.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub label: String,
    pub used: f64,
    /// None means unlimited.
    pub limit: Option<f64>,
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub provider_id: String,
    pub provider_name: String,
    /// None for the placeholder snapshot of a provider with no accounts yet.
    pub account_id: Option<String>,
    pub account: Option<String>,
    pub plan: Option<String>,
    pub windows: Vec<UsageWindow>,
    pub error: Option<String>,
    pub needs_auth: bool,
    /// Credentials are owned by another tool (e.g. Codex CLI); the app can't sign in or out.
    pub managed: bool,
    pub login_hint: Option<String>,
    /// Informational message when there is data but no usable windows.
    pub note: Option<String>,
    /// End of the current billing period when no window carries it.
    pub period_ends_at: Option<String>,
    /// Per-model billing details; only set when the account has a billing token.
    pub billing: Option<Billing>,
    /// A billing token is stored for this account, regardless of whether this fetch used it.
    pub billing_configured: bool,
    pub fetched_at: String,
}

/// Usage for one model in the current billing month.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub model: String,
    /// Credits covered by the plan.
    pub included: f64,
    pub included_amount: f64,
    /// Credits billed beyond the plan.
    pub additional: f64,
    pub additional_amount: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Billing {
    /// Sorted by total usage, largest first.
    pub models: Vec<ModelUsage>,
    pub additional_amount: f64,
    pub error: Option<String>,
}

impl UsageSnapshot {
    pub fn empty(p: &dyn Provider, account_id: Option<&str>) -> Self {
        Self {
            provider_id: p.id().into(),
            provider_name: p.name().into(),
            account_id: account_id.map(Into::into),
            account: account_id.map(Into::into),
            plan: None,
            windows: vec![],
            error: None,
            needs_auth: false,
            managed: false,
            login_hint: p.login_hint().map(Into::into),
            note: None,
            period_ends_at: None,
            billing: None,
            billing_configured: false,
            fetched_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Lowest remaining ratio across limited windows, in 0..=1.
    /// Matches the big number shown on the card.
    pub fn min_remaining(&self) -> Option<f64> {
        self.windows
            .iter()
            .filter_map(|w| w.limit.filter(|l| *l > 0.0).map(|l| (1.0 - w.used / l).clamp(0.0, 1.0)))
            .reduce(f64::min)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub interval: u64,
}

/// Result of a successful login: a stable account id (e.g. GitHub login) and its secret.
pub struct Credential {
    pub account_id: String,
    pub secret: String,
}

/// Providers are stateless; credential storage is handled by the app.
#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    /// Shown instead of a sign-in button for providers that can't log in from the app.
    fn login_hint(&self) -> Option<&'static str> {
        None
    }
    /// Accounts found on this machine (e.g. a CLI's credential file). Not persisted.
    fn discover(&self) -> Vec<Credential> {
        vec![]
    }
    async fn fetch(&self, http: &reqwest::Client, account_id: &str, secret: &str) -> UsageSnapshot;
    async fn start_login(&self, http: &reqwest::Client) -> Result<DeviceCode, String>;
    /// Polls until the user authorizes.
    async fn finish_login(&self, http: &reqwest::Client, code: DeviceCode) -> Result<Credential, String>;
}

pub fn registry() -> Vec<Box<dyn Provider>> {
    vec![Box::new(copilot::Copilot), Box::new(codex::Codex), Box::new(grok::Grok), Box::new(claude::Claude)]
}
