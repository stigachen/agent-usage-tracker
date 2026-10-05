//! Read-only selection of Grok CLI credentials. Never fall through to another
//! account just because the preferred entry is expired or malformed.

use super::CredentialIssue;
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use serde_json::Value;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const OIDC_PREFIX: &str = "https://auth.x.ai::";
const DEFAULT_SCOPE: &str = "https://auth.x.ai::b1a00492-073a-47ea-816f-4c329264a828";
const LEGACY_SCOPE: &str = "https://accounts.x.ai/sign-in";

// Deliberately no Debug/Serialize: this value contains a bearer token.
pub(super) struct Auth {
    pub key: String,
    identity: Identity,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(PartialEq, Eq)]
struct Identity {
    scope: String,
    email: Option<String>,
    user_id: Option<String>,
    principal_type: Option<String>,
    principal_id: Option<String>,
    team_id: Option<String>,
}

#[derive(Deserialize)]
struct Entry {
    key: String,
    email: Option<String>,
    user_id: Option<String>,
    principal_type: Option<String>,
    principal_id: Option<String>,
    team_id: Option<String>,
    oidc_issuer: Option<String>,
    oidc_client_id: Option<String>,
    expires_at: Option<DateTime<Utc>>,
    create_time: Option<DateTime<Utc>>,
}

impl Auth {
    pub fn account_id(&self) -> &str {
        // parse() requires an identity. Keep email-based IDs for existing tray/visibility settings.
        self.identity.email.as_deref().or(self.identity.user_id.as_deref()).unwrap()
    }

    pub fn expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.is_some_and(|expiry| expiry <= now)
    }

    pub fn same_account(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AuthError {
    Missing,
    Unreadable,
    Invalid,
    Unsupported,
    Ambiguous,
}

impl AuthError {
    pub fn issue(self) -> CredentialIssue {
        match self {
            Self::Missing => CredentialIssue::Missing,
            Self::Unreadable => CredentialIssue::Unreadable,
            Self::Invalid => CredentialIssue::Invalid,
            Self::Unsupported => CredentialIssue::Unsupported,
            Self::Ambiguous => CredentialIssue::Ambiguous,
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::Missing => "Grok CLI credentials were not found. Run `grok login`, then refresh.",
            Self::Unreadable => "Couldn't read Grok CLI credentials. Check the file permissions and GROK_HOME.",
            Self::Invalid => "Grok CLI credentials are incomplete or invalid. Run `grok login`, then refresh.",
            Self::Unsupported => "No supported Grok CLI login was found. Run `grok login`, then refresh.",
            Self::Ambiguous => "Multiple Grok CLI credential entries were found. The active account could not be determined.",
        }
    }
}

pub(super) fn path() -> Result<PathBuf, AuthError> {
    resolve_path(std::env::var_os("GROK_HOME"), dirs::home_dir())
}

fn resolve_path(custom: Option<OsString>, home: Option<PathBuf>) -> Result<PathBuf, AuthError> {
    let directory = custom.filter(|value| !value.is_empty()).map(PathBuf::from)
        .or_else(|| home.map(|path| path.join(".grok")))
        .ok_or(AuthError::Unreadable)?;
    Ok(directory.join("auth.json"))
}

pub(super) fn read(path: &Path) -> Result<Auth, AuthError> {
    let bytes = std::fs::read(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound { AuthError::Missing } else { AuthError::Unreadable }
    })?;
    parse(&bytes)
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.map(|value| value.trim().to_owned()).filter(|value| !value.is_empty())
}

fn parse(bytes: &[u8]) -> Result<Auth, AuthError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| AuthError::Invalid)?;
    let entries = value.as_object().ok_or(AuthError::Invalid)?;
    if entries.is_empty() {
        return Err(AuthError::Missing);
    }

    // The standard CLI client wins over stale legacy/other-client records.
    // A future CLI client is usable when it is the only xAI OIDC candidate.
    let scope = if entries.contains_key(DEFAULT_SCOPE) {
        DEFAULT_SCOPE
    } else {
        let mut oidc = entries.keys().filter(|scope| {
            scope.strip_prefix(OIDC_PREFIX).is_some_and(|client| !client.is_empty())
        });
        match (oidc.next(), oidc.next()) {
            (Some(scope), None) => scope.as_str(),
            (Some(_), Some(_)) => return Err(AuthError::Ambiguous),
            _ if entries.contains_key(LEGACY_SCOPE) => LEGACY_SCOPE,
            _ => return Err(AuthError::Unsupported),
        }
    };
    // Decode only the selected entry: an unrelated malformed record must not hide a valid login.
    let entry: Entry = serde_json::from_value(entries[scope].clone()).map_err(|_| AuthError::Invalid)?;
    if entry.key.is_empty() || entry.key.chars().any(char::is_whitespace) {
        return Err(AuthError::Invalid);
    }
    if let Some(client) = scope.strip_prefix(OIDC_PREFIX) {
        if entry.oidc_issuer.as_deref().is_some_and(|issuer| issuer.trim_end_matches('/') != "https://auth.x.ai")
            || entry.oidc_client_id.as_deref().is_some_and(|id| id != client)
        {
            return Err(AuthError::Invalid);
        }
    }
    let email = nonempty(entry.email);
    let user_id = nonempty(entry.user_id);
    if email.is_none() && user_id.is_none() {
        return Err(AuthError::Invalid);
    }
    // The CLI uses a 30-day lifetime for older credentials without an explicit expiry.
    let expires_at = match (entry.expires_at, entry.create_time) {
        (Some(expiry), _) => Some(expiry),
        (None, Some(created)) => Some(created.checked_add_signed(Duration::days(30)).ok_or(AuthError::Invalid)?),
        _ => None,
    };
    Ok(Auth {
        key: entry.key,
        identity: Identity {
            scope: scope.to_owned(),
            email,
            user_id,
            principal_type: nonempty(entry.principal_type),
            principal_id: nonempty(entry.principal_id),
            team_id: nonempty(entry.team_id),
        },
        expires_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(email: &str) -> Value {
        json!({ "key": "test-token", "email": email, "user_id": "user-1" })
    }

    fn parse_value(value: Value) -> Result<Auth, AuthError> {
        parse(&serde_json::to_vec(&value).unwrap())
    }

    #[test]
    fn uses_custom_home_without_requiring_default_home() {
        let custom = PathBuf::from("Grok 用户 profiles").join("personal");
        assert_eq!(resolve_path(Some(custom.clone().into_os_string()), None).unwrap(), custom.join("auth.json"));
        let home = PathBuf::from("user-home");
        for custom in [None, Some(OsString::new())] {
            assert_eq!(resolve_path(custom, Some(home.clone())).unwrap(), home.join(".grok/auth.json"));
        }
        assert_eq!(resolve_path(None, None), Err(AuthError::Unreadable));
    }

    #[test]
    fn prefers_standard_cli_scope_over_other_clients_and_legacy() {
        let auth = parse_value(json!({
            LEGACY_SCOPE: entry("legacy@example.com"),
            "https://auth.x.ai::another-client": entry("other@example.com"),
            DEFAULT_SCOPE: entry("current@example.com"),
            "unrelated": { "key": 42 },
        })).unwrap();
        assert_eq!(auth.account_id(), "current@example.com");
    }

    #[test]
    fn accepts_one_oidc_scope_or_exact_legacy_scope() {
        let auth = parse_value(json!({
            "https://auth.x.ai::future-client": entry("oidc@example.com"),
            LEGACY_SCOPE: entry("legacy@example.com"),
        })).unwrap();
        assert_eq!(auth.account_id(), "oidc@example.com");
        let auth = parse_value(json!({ LEGACY_SCOPE: entry("legacy@example.com") })).unwrap();
        assert_eq!(auth.account_id(), "legacy@example.com");
    }

    #[test]
    fn refuses_to_guess_between_nonstandard_oidc_scopes() {
        assert!(matches!(parse_value(json!({
            "https://auth.x.ai::client-a": entry("a@example.com"),
            "https://auth.x.ai::client-b": entry("b@example.com"),
            LEGACY_SCOPE: entry("legacy@example.com"),
        })), Err(AuthError::Ambiguous)));
    }

    #[test]
    fn never_substitutes_another_account_for_an_expired_or_invalid_entry() {
        let mut expired = entry("expired@example.com");
        expired["expires_at"] = json!("2000-01-01T00:00:00Z");
        let auth = parse_value(json!({ DEFAULT_SCOPE: expired, LEGACY_SCOPE: entry("legacy@example.com") })).unwrap();
        assert_eq!(auth.account_id(), "expired@example.com");
        assert!(auth.expired(Utc::now()));

        for invalid in [json!({}), json!({ "key": "", "email": "invalid@example.com" }), json!(null)] {
            assert!(matches!(parse_value(json!({
                DEFAULT_SCOPE: invalid, LEGACY_SCOPE: entry("legacy@example.com"),
            })), Err(AuthError::Invalid)));
        }
    }

    #[test]
    fn rejects_foreign_scopes_and_mismatched_issuer_metadata() {
        for scope in ["https://auth.x.ai.evil.test::client", "https://other.test/sign-in", "xai::api_key"] {
            assert!(matches!(parse_value(json!({ scope: entry("other@example.com") })), Err(AuthError::Unsupported)));
        }
        // The CLI trims trailing slashes when building a scope from its issuer URL.
        let mut valid = entry("account@example.com");
        valid["oidc_issuer"] = json!("https://auth.x.ai/");
        valid["oidc_client_id"] = json!(DEFAULT_SCOPE.strip_prefix(OIDC_PREFIX).unwrap());
        assert!(parse_value(json!({ DEFAULT_SCOPE: valid })).is_ok());
        for (field, value) in [("oidc_issuer", "https://other.test"), ("oidc_client_id", "wrong-client")] {
            let mut invalid = entry("invalid@example.com");
            invalid[field] = json!(value);
            assert!(matches!(parse_value(json!({ DEFAULT_SCOPE: invalid })), Err(AuthError::Invalid)));
        }
    }

    #[test]
    fn distinguishes_missing_from_invalid_contents() {
        assert!(matches!(parse(b"{}"), Err(AuthError::Missing)));
        for bytes in [b"".as_slice(), b"{", b"[]", b"null"] {
            assert!(matches!(parse(bytes), Err(AuthError::Invalid)));
        }
        for invalid in [
            json!({ "key": "secret\nvalue", "email": "a@example.com" }),
            json!({ "key": "test-token", "email": " ", "user_id": "" }),
            json!({ "key": "test-token", "email": "a@example.com", "expires_at": "bad-date" }),
        ] {
            assert!(matches!(parse_value(json!({ DEFAULT_SCOPE: invalid })), Err(AuthError::Invalid)));
        }
    }

    #[test]
    fn preserves_existing_email_ids_and_can_identify_email_less_credentials() {
        let auth = parse_value(json!({ DEFAULT_SCOPE: entry("  account@example.com  ") })).unwrap();
        assert_eq!(auth.account_id(), "account@example.com");
        let auth = parse_value(json!({ DEFAULT_SCOPE: { "key": "test-token", "user_id": "user-1" } })).unwrap();
        assert_eq!(auth.account_id(), "user-1");
    }

    #[test]
    fn uses_explicit_expiry_and_legacy_thirty_day_lifetime() {
        let now = DateTime::parse_from_rfc3339("2026-10-05T00:00:00Z").unwrap().with_timezone(&Utc);
        let mut value = entry("account@example.com");
        value["create_time"] = json!("2026-01-01T00:00:00Z");
        value["expires_at"] = json!("2026-10-05T00:00:00Z");
        let auth = parse_value(json!({ DEFAULT_SCOPE: value })).unwrap();
        assert!(!auth.expired(now - Duration::seconds(1)));
        assert!(auth.expired(now));

        let mut value = entry("account@example.com");
        value["create_time"] = json!("2026-09-05T00:00:00Z");
        let auth = parse_value(json!({ DEFAULT_SCOPE: value })).unwrap();
        assert!(!auth.expired(now - Duration::seconds(1)));
        assert!(auth.expired(now));
        assert!(!parse_value(json!({ DEFAULT_SCOPE: entry("legacy@example.com") })).unwrap().expired(now));
    }

    #[test]
    fn identity_tracks_principal_and_user_but_not_bearer_rotation() {
        let value = entry("same@example.com");
        let original = parse_value(json!({ DEFAULT_SCOPE: value.clone() })).unwrap();
        let mut rotated = value.clone();
        rotated["key"] = json!("rotated-token");
        assert!(original.same_account(&parse_value(json!({ DEFAULT_SCOPE: rotated })).unwrap()));
        for field in ["email", "user_id", "principal_type", "principal_id", "team_id"] {
            let mut changed = value.clone();
            changed[field] = json!("different");
            assert!(!original.same_account(&parse_value(json!({ DEFAULT_SCOPE: changed })).unwrap()));
        }
    }
}
