//! Grok (SuperGrok / X Premium) usage via the Grok CLI's billing proxy.
//! Credentials are owned and refreshed by the `grok` CLI in `~/.grok/auth.json`; read-only here.

use super::{Credential, DeviceCode, Provider, UsageSnapshot, UsageWindow};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::PathBuf;

const PROXY: &str = "https://cli-chat-proxy.grok.com/v1";

pub struct Grok;

#[derive(Deserialize)]
struct AuthEntry {
    key: String,
    email: Option<String>,
    expires_at: Option<String>,
}

#[derive(Deserialize)]
struct BillingResp {
    config: Option<BillingConfig>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BillingConfig {
    credit_usage_percent: Option<f64>,
    on_demand_used: Option<Val>,
    on_demand_cap: Option<Val>,
    current_period: Option<Period>,
    billing_period_end: Option<String>,
}

#[derive(Deserialize)]
struct Val {
    val: Option<f64>,
}

#[derive(Deserialize)]
struct Period {
    #[serde(rename = "type")]
    kind: Option<String>,
    end: Option<String>,
}

#[derive(Deserialize)]
struct SettingsResp {
    subscription_tier_display: Option<String>,
}

fn auth_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".grok/auth.json"))
}

/// auth.json is a map keyed by issuer::client; take the first entry.
fn read_auth() -> Option<AuthEntry> {
    let bytes = std::fs::read(auth_path()?).ok()?;
    let map: std::collections::HashMap<String, AuthEntry> = serde_json::from_slice(&bytes).ok()?;
    map.into_values().next()
}

fn expired(a: &AuthEntry) -> bool {
    a.expires_at
        .as_deref()
        .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        .is_some_and(|t| t < chrono::Utc::now())
}

fn period_label(kind: Option<&str>) -> &'static str {
    match kind {
        Some(k) if k.contains("WEEKLY") => "Weekly limit",
        Some(k) if k.contains("MONTHLY") => "Monthly limit",
        Some(k) if k.contains("DAILY") => "Daily limit",
        _ => "Usage limit",
    }
}

#[async_trait]
impl Provider for Grok {
    fn id(&self) -> &'static str {
        "grok"
    }
    fn name(&self) -> &'static str {
        "Grok"
    }
    fn login_hint(&self) -> Option<&'static str> {
        Some("Run `grok login` (or any grok command to refresh), then refresh.")
    }

    fn discover(&self) -> Vec<Credential> {
        read_auth()
            .and_then(|a| a.email)
            .map(|account_id| Credential { account_id, secret: String::new() })
            .into_iter()
            .collect()
    }

    async fn fetch(&self, http: &reqwest::Client, account_id: &str, _secret: &str) -> UsageSnapshot {
        let mut snap = UsageSnapshot::empty(self, Some(account_id));
        snap.managed = true;
        let Some(auth) = read_auth() else {
            snap.needs_auth = true;
            return snap;
        };
        if expired(&auth) {
            snap.needs_auth = true;
            return snap;
        }
        let get = |path: &str| {
            http.get(format!("{PROXY}{path}"))
                .bearer_auth(&auth.key)
                .header("x-xai-token-auth", "xai-grok-cli")
                .header("Accept", "application/json")
        };

        let body: BillingResp = match get("/billing?format=credits").send().await {
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

        // Plan is optional enrichment; ignore failures.
        if let Ok(r) = get("/settings").send().await {
            if let Ok(s) = r.json::<SettingsResp>().await {
                snap.plan = s.subscription_tier_display;
            }
        }

        let Some(cfg) = body.config else {
            snap.note = Some("Grok didn't report usage for this account.".into());
            return snap;
        };
        let cap = cfg.on_demand_cap.and_then(|v| v.val).filter(|c| *c > 0.0);
        let pct = cfg.credit_usage_percent.or_else(|| {
            let used = cfg.on_demand_used.and_then(|v| v.val)?;
            cap.map(|c| used / c * 100.0)
        });
        let kind = cfg.current_period.as_ref().and_then(|p| p.kind.clone());
        let resets_at = cfg
            .current_period
            .and_then(|p| p.end)
            .or(cfg.billing_period_end);
        // The proxy omits the percent for some plans; grok.com's billing RPC carries it.
        let pct = match pct {
            Some(p) => Some(p),
            None => grpc_usage_percent(http, &auth.key).await,
        };
        match pct {
            Some(p) => {
                snap.windows = vec![UsageWindow {
                    label: period_label(kind.as_deref()).into(),
                    used: p.clamp(0.0, 100.0),
                    limit: Some(100.0),
                    resets_at,
                }]
            }
            // Don't invent a number: the proxy only sends the billing period for some plans.
            None => {
                snap.note = Some("Grok doesn't report a usage percentage for this plan yet.".into());
                snap.period_ends_at = resets_at;
            }
        }
        snap
    }

    async fn start_login(&self, _http: &reqwest::Client) -> Result<DeviceCode, String> {
        Err("Grok accounts come from the Grok CLI. Run `grok login`.".into())
    }

    async fn finish_login(&self, _http: &reqwest::Client, _code: DeviceCode) -> Result<Credential, String> {
        Err("unsupported".into())
    }
}

/// Calls grok.com `GetGrokCreditsConfig` (gRPC-web) and returns used percent.
/// proto3 omits a zero float, so an active period with no percent field means 0%.
async fn grpc_usage_percent(http: &reqwest::Client, token: &str) -> Option<f64> {
    let body = http
        .post("https://grok.com/grok_api_v2.GrokBuildBilling/GetGrokCreditsConfig")
        .bearer_auth(token)
        .header("Content-Type", "application/grpc-web+proto")
        .header("x-grpc-web", "1")
        .header("Origin", "https://grok.com")
        // Frame: uncompressed, len 2, message { field1: false }.
        .body(vec![0u8, 0, 0, 0, 2, 8, 0])
        .timeout(std::time::Duration::from_secs(6))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .bytes()
        .await
        .ok()?;
    // First data frame: flag(1) + len(4) + payload.
    if body.len() < 5 || body[0] != 0 {
        return None;
    }
    let len = u32::from_be_bytes(body[1..5].try_into().ok()?) as usize;
    let payload = body.get(5..5 + len)?;
    let config = pb::fields(payload)?.into_iter().find(|f| f.num == 1)?.bytes?;
    let fields = pb::fields(config)?;
    if let Some(p) = fields.iter().find(|f| f.num == 1).and_then(|f| f.fixed32) {
        let p = f32::from_bits(p) as f64;
        return (0.0..=100.0).contains(&p).then_some(p);
    }
    // Field 8 = current period { 1: type, 2: start ts, 3: end ts }.
    let period = pb::fields(fields.iter().find(|f| f.num == 8)?.bytes?)?;
    let ts = |n| {
        let m = pb::fields(period.iter().find(|f| f.num == n)?.bytes?)?;
        m.iter().find(|f| f.num == 1)?.varint.map(|v| v as i64)
    };
    let now = chrono::Utc::now().timestamp();
    (ts(2)? <= now && ts(3)? > now).then_some(0.0)
}

/// Just enough protobuf decoding for the billing response.
mod pb {
    pub struct Field<'a> {
        pub num: u64,
        pub varint: Option<u64>,
        pub fixed32: Option<u32>,
        pub bytes: Option<&'a [u8]>,
    }

    fn varint(b: &[u8], i: &mut usize) -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = *b.get(*i)?;
            *i += 1;
            v |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }

    pub fn fields(b: &[u8]) -> Option<Vec<Field<'_>>> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < b.len() {
            let key = varint(b, &mut i)?;
            let mut f = Field { num: key >> 3, varint: None, fixed32: None, bytes: None };
            match key & 7 {
                0 => f.varint = Some(varint(b, &mut i)?),
                1 => i += 8,
                2 => {
                    let n = varint(b, &mut i)? as usize;
                    f.bytes = Some(b.get(i..i + n)?);
                    i += n;
                }
                5 => {
                    f.fixed32 = Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?));
                    i += 4;
                }
                _ => return None,
            }
            out.push(f);
        }
        Some(out)
    }
}
