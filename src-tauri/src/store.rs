//! Non-secret app config (account list, tray preference) persisted as JSON.
//! Secrets live in the OS keychain under `<provider>:<account>`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const KEYRING_SERVICE: &str = "com.stigachen.agentusage";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountRef {
    pub provider: String,
    pub id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "mode")]
pub enum TrayDisplay {
    /// Remaining % of whichever account is closest to running out.
    #[default]
    Lowest,
    /// Remaining % of one specific account.
    Pinned { provider: String, account: String },
    IconOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub accounts: Vec<AccountRef>,
    pub tray_display: TrayDisplay,
    pub refresh_secs: u64,
    /// Provider ids in the user's order; unlisted providers follow in registry order.
    pub provider_order: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self { accounts: vec![], tray_display: TrayDisplay::default(), refresh_secs: 600, provider_order: vec![] }
    }
}

pub struct Store {
    path: PathBuf,
    pub config: Config,
}

impl Store {
    pub fn load(dir: PathBuf) -> Self {
        let path = dir.join("config.json");
        let config = std::fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self { path, config }
    }

    pub fn save(&self) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_vec_pretty(&self.config).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, json).map_err(|e| e.to_string())
    }
}

fn entry(key: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, key)
}

fn key(provider: &str, account: &str) -> String {
    format!("{provider}:{account}")
}

pub fn get_secret(provider: &str, account: &str) -> Option<String> {
    read_secret(provider, account).ok().flatten()
}

/// Like `get_secret`, but tells "no token stored" (`Ok(None)`) apart from a keychain failure.
pub fn read_secret(provider: &str, account: &str) -> Result<Option<String>, String> {
    match entry(&key(provider, account)).and_then(|e| e.get_password()) {
        Ok(s) => Ok(Some(s)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn set_secret(provider: &str, account: &str, secret: &str) -> Result<(), String> {
    entry(&key(provider, account))
        .and_then(|e| e.set_password(secret))
        .map_err(|e| e.to_string())
}

pub fn delete_secret(provider: &str, account: &str) -> Result<(), String> {
    match entry(&key(provider, account)).and_then(|e| e.delete_credential()) {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Takes the pre-multi-account token stored under the bare provider id, removing it.
pub fn take_legacy_secret(provider: &str) -> Option<String> {
    let e = entry(provider).ok()?;
    let secret = e.get_password().ok()?;
    let _ = e.delete_credential();
    Some(secret)
}
