pub mod copilot;

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
    pub fetched_at: String,
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
    async fn fetch(&self, http: &reqwest::Client, account_id: &str, secret: &str) -> UsageSnapshot;
    async fn start_login(&self, http: &reqwest::Client) -> Result<DeviceCode, String>;
    /// Polls until the user authorizes.
    async fn finish_login(&self, http: &reqwest::Client, code: DeviceCode) -> Result<Credential, String>;
}

pub fn registry() -> Vec<Box<dyn Provider>> {
    vec![Box::new(copilot::Copilot)]
}
