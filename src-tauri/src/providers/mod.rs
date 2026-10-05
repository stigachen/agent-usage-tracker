pub mod claude;
pub mod codex;
pub mod copilot;
pub mod grok;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Optional detail for providers whose credentials are managed by another app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialIssue {
    Missing,
    Unreadable,
    Invalid,
    Unsupported,
    Ambiguous,
    Expired,
    Rejected,
    Changed,
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_issue: Option<CredentialIssue>,
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
    /// The user hid this account from the overview.
    pub hidden: bool,
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
            credential_issue: None,
            managed: false,
            login_hint: p.login_hint().map(Into::into),
            note: None,
            period_ends_at: None,
            billing: None,
            billing_configured: false,
            hidden: false,
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
    /// Fetch local accounts, or return None to use the app's stored accounts.
    /// Providers may override this to capture CLI credentials once and report read failures.
    async fn fetch_discovered(&self, http: &reqwest::Client) -> Option<Vec<UsageSnapshot>> {
        let credentials = self.discover();
        if credentials.is_empty() {
            return None;
        }
        let mut snapshots = Vec::with_capacity(credentials.len());
        for credential in credentials {
            snapshots.push(self.fetch(http, &credential.account_id, &credential.secret).await);
        }
        Some(snapshots)
    }
    async fn fetch(&self, http: &reqwest::Client, account_id: &str, secret: &str) -> UsageSnapshot;
    async fn start_login(&self, http: &reqwest::Client) -> Result<DeviceCode, String>;
    /// Polls until the user authorizes.
    async fn finish_login(&self, http: &reqwest::Client, code: DeviceCode) -> Result<Credential, String>;
}

pub fn registry() -> Vec<Box<dyn Provider>> {
    vec![Box::new(copilot::Copilot), Box::new(codex::Codex), Box::new(grok::Grok), Box::new(claude::Claude)]
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ExistingProvider {
        has_local_accounts: bool,
    }

    #[async_trait]
    impl Provider for ExistingProvider {
        fn id(&self) -> &'static str { "existing" }
        fn name(&self) -> &'static str { "Existing provider" }
        fn discover(&self) -> Vec<Credential> {
            if !self.has_local_accounts { return vec![]; }
            vec![
                Credential { account_id: "first".into(), secret: "first-secret".into() },
                Credential { account_id: "second".into(), secret: "second-secret".into() },
            ]
        }
        async fn fetch(&self, _http: &reqwest::Client, account_id: &str, secret: &str) -> UsageSnapshot {
            assert_eq!(secret, format!("{account_id}-secret"));
            let mut snapshot = UsageSnapshot::empty(self, Some(account_id));
            if account_id == "second" { snapshot.error = Some("test fetch failure".into()); }
            snapshot
        }
        async fn start_login(&self, _http: &reqwest::Client) -> Result<DeviceCode, String> {
            unreachable!()
        }
        async fn finish_login(&self, _http: &reqwest::Client, _code: DeviceCode) -> Result<Credential, String> {
            unreachable!()
        }
    }

    #[tokio::test]
    async fn default_discovery_keeps_account_secret_pairing_order_and_fetch_errors() {
        let provider = ExistingProvider { has_local_accounts: true };
        let snapshots = provider.fetch_discovered(&reqwest::Client::new()).await.unwrap();
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].account_id.as_deref(), Some("first"));
        assert!(snapshots[0].error.is_none());
        assert_eq!(snapshots[1].account_id.as_deref(), Some("second"));
        assert_eq!(snapshots[1].error.as_deref(), Some("test fetch failure"));
    }

    #[tokio::test]
    async fn default_discovery_preserves_stored_account_fallback() {
        let provider = ExistingProvider { has_local_accounts: false };
        assert!(provider.fetch_discovered(&reqwest::Client::new()).await.is_none());
    }
}
