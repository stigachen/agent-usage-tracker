//! Non-secret app config (account list, tray preference) persisted as JSON.
//! Secrets live in the OS keychain under `<provider>:<account>`.

use crate::localization::Language;
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
    pub language: Language,
    pub accounts: Vec<AccountRef>,
    pub tray_display: TrayDisplay,
    pub refresh_secs: u64,
    /// Provider ids in the user's order; unlisted providers follow in registry order.
    pub provider_order: Vec<String>,
    /// Accounts kept out of the overview (and the tray's "lowest"). Covers discovered
    /// accounts too, which aren't in `accounts`.
    pub hidden: Vec<AccountRef>,
}

impl Default for Config {
    fn default() -> Self {
        Self { language: Language::default(), accounts: vec![], tray_display: TrayDisplay::default(), refresh_secs: 600, provider_order: vec![], hidden: vec![] }
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

    pub fn set_language(&mut self, language: Language) -> Result<(), String> {
        let previous = self.config.language;
        self.config.language = language;
        if let Err(error) = self.save() {
            self.config.language = previous;
            return Err(error);
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_configs_default_to_system_without_losing_existing_preferences() {
        let json = r#"{"accounts":[{"provider":"copilot","id":"alice"}],"trayDisplay":{"mode":"pinned","provider":"copilot","account":"alice"},"refreshSecs":300,"providerOrder":["codex","copilot"],"hidden":[{"provider":"codex","id":"bob"}]}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.language, Language::System);
        let mut saved = serde_json::to_value(&config).unwrap();
        saved.as_object_mut().unwrap().remove("language");
        assert_eq!(saved, serde_json::from_str::<serde_json::Value>(json).unwrap());

        let mut future: serde_json::Value = serde_json::from_str(json).unwrap();
        future["language"] = "fr".into();
        let config: Config = serde_json::from_value(future).unwrap();
        assert_eq!(config.language, Language::System);
        assert_eq!(config.accounts[0].id, "alice");
        assert_eq!(config.refresh_secs, 300);
    }

    #[test]
    fn language_persists_and_failed_saves_keep_the_previous_choice() {
        let dir = std::env::temp_dir().join(format!("agent-usage-language-{}-{}", std::process::id(), chrono::Utc::now().timestamp_nanos_opt().unwrap()));
        let mut store = Store::load(dir.clone());
        store.config.refresh_secs = 300;
        for (language, value) in [(Language::System, "system"), (Language::English, "en"), (Language::SimplifiedChinese, "zh-CN")] {
            store.set_language(language).unwrap();
            assert_eq!(Store::load(dir.clone()).config.language, language);
            let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(&store.path).unwrap()).unwrap();
            assert_eq!(saved["language"], value);
            assert_eq!(saved["refreshSecs"], 300);
        }
        // An ordinary file can't be used as the parent directory of config.json.
        store.path = store.path.join("config.json");
        assert!(store.set_language(Language::English).is_err());
        assert_eq!(store.config.language, Language::SimplifiedChinese);
        assert_eq!(Store::load(dir.clone()).config.language, Language::SimplifiedChinese);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
