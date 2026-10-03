mod providers;
mod store;

use providers::{DeviceCode, Provider, UsageSnapshot};
use store::{AccountRef, Store, TrayDisplay};
use std::sync::Arc;
use std::time::Duration;
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WindowEvent,
};
use tauri_plugin_positioner::{Position, WindowExt};
use tokio::sync::{Notify, RwLock};

const TRAY_ID: &str = "main";

/// When the panel was last hidden by losing focus. Clicking the tray icon blurs the
/// panel before the click event arrives, so without this the click would reopen it.
static LAST_BLUR_HIDE: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

struct AppState {
    http: reqwest::Client,
    providers: Vec<Box<dyn Provider>>,
    snapshots: RwLock<Vec<UsageSnapshot>>,
    store: RwLock<Store>,
    wake: Notify,
    /// Bumped on every billing token change per account, so a slow save can't undo a later remove.
    billing_ops: std::sync::Mutex<std::collections::HashMap<String, u64>>,
}

type Shared = Arc<AppState>;

impl AppState {
    fn bump_billing_op(&self, account: &str) -> u64 {
        let mut ops = self.billing_ops.lock().unwrap();
        let v = ops.entry(account.to_string()).or_default();
        *v += 1;
        *v
    }

    fn provider(&self, id: &str) -> Result<&dyn Provider, String> {
        self.providers
            .iter()
            .find(|p| p.id() == id)
            .map(|p| p.as_ref())
            .ok_or_else(|| format!("unknown provider {id}"))
    }
}

/// Providers in the user's order, then any not yet ordered in registry order.
fn ordered_providers<'a>(state: &'a AppState, order: &[String]) -> Vec<&'a dyn Provider> {
    let mut ps: Vec<&dyn Provider> = state.providers.iter().map(|p| p.as_ref()).collect();
    ps.sort_by_key(|p| order.iter().position(|id| id == p.id()).unwrap_or(usize::MAX));
    ps
}

/// Sorts snapshots by provider order, then by the stored account order. Stable, so
/// discovered accounts (not in the stored list) keep their relative order.
fn sort_snapshots(snaps: &mut [UsageSnapshot], state: &AppState, cfg: &store::Config) {
    let ps = ordered_providers(state, &cfg.provider_order);
    let rank = |s: &UsageSnapshot| {
        let p = ps.iter().position(|p| p.id() == s.provider_id).unwrap_or(usize::MAX);
        let a = cfg
            .accounts
            .iter()
            .position(|a| a.provider == s.provider_id && Some(&a.id) == s.account_id.as_ref())
            .unwrap_or(usize::MAX);
        (p, a)
    };
    snaps.sort_by_key(rank);
}

/// Sorts and publishes snapshots. `fresh` replaces the cache; `None` re-sorts what's cached.
/// Everything happens under the snapshots write lock so a refresh and a re-sort can't
/// interleave and publish an older copy last.
async fn publish(app: &AppHandle, state: &AppState, fresh: Option<Vec<UsageSnapshot>>) {
    let cfg = state.store.read().await.config.clone();
    let mut cache = state.snapshots.write().await;
    if let Some(snaps) = fresh {
        *cache = snaps;
    }
    sort_snapshots(&mut cache, state, &cfg);
    for s in cache.iter_mut() {
        s.hidden = s.account_id.as_ref().is_some_and(|id| {
            cfg.hidden.iter().any(|h| h.provider == s.provider_id && &h.id == id)
        });
    }
    update_tray(app, &cache, &cfg.tray_display);
    let _ = app.emit("usage-updated", cache.clone());
}

async fn refresh_all(app: &AppHandle, state: &AppState) {
    let accounts = state.store.read().await.config.accounts.clone();
    let mut snaps = Vec::new();
    // Fetches sequentially; fine for a handful of accounts and avoids the `futures` crate.
    for p in &state.providers {
        let discovered = p.discover();
        if !discovered.is_empty() {
            for c in discovered {
                snaps.push(p.fetch(&state.http, &c.account_id, &c.secret).await);
            }
            continue;
        }
        let mine: Vec<_> = accounts.iter().filter(|a| a.provider == p.id()).collect();
        if mine.is_empty() {
            let mut s = UsageSnapshot::empty(p.as_ref(), None);
            s.needs_auth = true;
            snaps.push(s);
        }
        for a in mine {
            let snap = match store::read_secret(&a.provider, &a.id) {
                Ok(Some(secret)) => p.fetch(&state.http, &a.id, &secret).await,
                other => {
                    let mut s = UsageSnapshot::empty(p.as_ref(), Some(&a.id));
                    match other {
                        Err(e) => s.error = Some(format!("Couldn't read the token from the keychain: {e}")),
                        _ => s.needs_auth = true,
                    }
                    // Keep the billing token manageable even when the main token is unusable.
                    s.billing_configured =
                        a.provider == "copilot" && store::get_secret(providers::copilot::BILLING_KEY, &a.id).is_some();
                    s
                }
            };
            snaps.push(snap);
        }
    }
    publish(app, state, Some(snaps)).await;
}

/// Re-applies order and visibility to the cached snapshots, without refetching.
async fn resort(app: &AppHandle, state: &AppState) {
    publish(app, state, None).await;
}

#[tauri::command]
async fn set_provider_order(app: AppHandle, state: State<'_, Shared>, order: Vec<String>) -> Result<(), String> {
    {
        let mut st = state.store.write().await;
        st.config.provider_order = order;
        st.save()?;
    }
    resort(&app, &state).await;
    Ok(())
}

#[tauri::command]
async fn set_account_hidden(
    app: AppHandle,
    state: State<'_, Shared>,
    provider: String,
    account: String,
    hidden: bool,
) -> Result<(), String> {
    {
        let mut st = state.store.write().await;
        let acc = AccountRef { provider, id: account };
        st.config.hidden.retain(|a| *a != acc);
        if hidden {
            st.config.hidden.push(acc);
        }
        st.save()?;
    }
    resort(&app, &state).await;
    Ok(())
}

/// Reorders one provider's stored accounts; `ids` lists them in the new order.
#[tauri::command]
async fn set_account_order(
    app: AppHandle,
    state: State<'_, Shared>,
    provider: String,
    ids: Vec<String>,
) -> Result<(), String> {
    {
        let mut st = state.store.write().await;
        let accounts = &mut st.config.accounts;
        // Fill this provider's slots, in place, with its accounts in the requested order.
        let mut mine: Vec<AccountRef> = accounts.iter().filter(|a| a.provider == provider).cloned().collect();
        mine.sort_by_key(|a| ids.iter().position(|id| *id == a.id).unwrap_or(usize::MAX));
        let mut next = mine.into_iter();
        for a in accounts.iter_mut().filter(|a| a.provider == provider) {
            *a = next.next().unwrap();
        }
        st.save()?;
    }
    resort(&app, &state).await;
    Ok(())
}

/// Moves tokens saved before multi-account support to `<provider>:<login>`.
async fn migrate_legacy(state: &AppState) {
    for p in &state.providers {
        let Some(secret) = store::take_legacy_secret(p.id()) else { continue };
        let snap = p.fetch(&state.http, "", &secret).await;
        let Some(login) = snap.account.filter(|a| !a.is_empty()) else { continue };
        if store::set_secret(p.id(), &login, &secret).is_ok() {
            add_account(state, p.id(), &login).await;
        }
    }
}

async fn add_account(state: &AppState, provider: &str, id: &str) {
    let mut st = state.store.write().await;
    let acc = AccountRef { provider: provider.into(), id: id.into() };
    if !st.config.accounts.contains(&acc) {
        st.config.accounts.push(acc);
        let _ = st.save();
    }
}

fn tray_title(snaps: &[UsageSnapshot], display: &TrayDisplay) -> Option<String> {
    let remaining = match display {
        TrayDisplay::IconOnly => None,
        TrayDisplay::Lowest => snaps.iter().filter(|s| !s.hidden).filter_map(|s| s.min_remaining()).reduce(f64::min),
        TrayDisplay::Pinned { provider, account } => snaps
            .iter()
            .find(|s| &s.provider_id == provider && s.account_id.as_deref() == Some(account))
            .and_then(|s| s.min_remaining()),
    };
    // Floor so the tray never rounds up past the card's big number.
    remaining.map(|r| format!("{:.0}%", (r * 100.0).floor()))
}

fn update_tray(app: &AppHandle, snaps: &[UsageSnapshot], display: &TrayDisplay) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let _ = tray.set_title(tray_title(snaps, display).as_deref());
}

fn toggle_panel(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else { return };
    let just_hidden = LAST_BLUR_HIDE
        .lock()
        .unwrap()
        .take()
        .is_some_and(|t| t.elapsed() < Duration::from_millis(300));
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
    } else if !just_hidden {
        let _ = win.move_window(Position::TrayCenter);
        let _ = win.show();
        let _ = win.set_focus();
        let _ = app.emit("panel-shown", ());
        // Refresh on open so numbers are never stale.
        app.state::<Shared>().wake.notify_one();
    }
}

#[tauri::command]
async fn get_snapshots(state: State<'_, Shared>) -> Result<Vec<UsageSnapshot>, ()> {
    Ok(state.snapshots.read().await.clone())
}

#[tauri::command]
fn refresh(state: State<'_, Shared>) {
    state.wake.notify_one();
}

#[tauri::command]
async fn start_login(state: State<'_, Shared>, provider: String) -> Result<DeviceCode, String> {
    state.provider(&provider)?.start_login(&state.http).await
}

#[tauri::command]
async fn finish_login(
    state: State<'_, Shared>,
    provider: String,
    code: DeviceCode,
) -> Result<(), String> {
    let cred = state.provider(&provider)?.finish_login(&state.http, code).await?;
    store::set_secret(&provider, &cred.account_id, &cred.secret)?;
    add_account(&state, &provider, &cred.account_id).await;
    state.wake.notify_one();
    Ok(())
}

#[tauri::command]
async fn logout(state: State<'_, Shared>, provider: String, account: String) -> Result<(), String> {
    // Remove the account even if the keychain refuses, so sign-out never gets stuck;
    // report the leftover entry afterwards.
    let mut leftover = store::delete_secret(&provider, &account).err();
    if provider == "copilot" {
        state.bump_billing_op(&account);
        if let Err(e) = store::delete_secret(providers::copilot::BILLING_KEY, &account) {
            leftover.get_or_insert(e);
        }
    }
    {
        let mut st = state.store.write().await;
        st.config.accounts.retain(|a| !(a.provider == provider && a.id == account));
        st.config.hidden.retain(|a| !(a.provider == provider && a.id == account));
        if matches!(&st.config.tray_display, TrayDisplay::Pinned { provider: p, account: a } if *p == provider && *a == account) {
            st.config.tray_display = TrayDisplay::Lowest;
        }
        st.save()?;
    }
    state.wake.notify_one();
    match leftover {
        Some(e) => Err(format!("Signed out, but the token couldn't be removed from the keychain: {e}")),
        None => Ok(()),
    }
}

/// Saves (or with an empty token, removes) a Copilot account's billing PAT after verifying it.
#[tauri::command]
async fn set_billing_token(state: State<'_, Shared>, account: String, token: String) -> Result<(), String> {
    let op = state.bump_billing_op(&account);
    let token = token.trim();
    if token.is_empty() {
        store::delete_secret(providers::copilot::BILLING_KEY, &account)?;
    } else {
        providers::copilot::verify_billing_token(&state.http, &account, token).await?;
        // Check and write under the lock so a remove can't slip in between.
        let ops = state.billing_ops.lock().unwrap();
        if ops.get(&account) != Some(&op) {
            return Err("Token was changed while this one was being checked".into());
        }
        store::set_secret(providers::copilot::BILLING_KEY, &account, token)?;
    }
    state.wake.notify_one();
    Ok(())
}

#[tauri::command]
async fn get_tray_display(state: State<'_, Shared>) -> Result<TrayDisplay, ()> {
    Ok(state.store.read().await.config.tray_display.clone())
}

#[tauri::command]
async fn set_tray_display(
    app: AppHandle,
    state: State<'_, Shared>,
    display: TrayDisplay,
) -> Result<(), String> {
    {
        let mut st = state.store.write().await;
        st.config.tray_display = display.clone();
        st.save()?;
    }
    update_tray(&app, &state.snapshots.read().await, &display);
    Ok(())
}

#[tauri::command]
async fn get_refresh_secs(state: State<'_, Shared>) -> Result<u64, ()> {
    Ok(state.store.read().await.config.refresh_secs)
}

#[tauri::command]
async fn set_refresh_secs(state: State<'_, Shared>, secs: u64) -> Result<(), String> {
    let mut st = state.store.write().await;
    st.config.refresh_secs = secs.clamp(60, 3600);
    st.save()?;
    // Restart the sleep so the new interval applies right away.
    state.wake.notify_one();
    Ok(())
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let cmd = ("open", vec![url]);
    #[cfg(target_os = "windows")]
    let cmd = ("cmd", vec!["/C".into(), "start".into(), "".into(), url]);
    std::process::Command::new(cmd.0)
        .args(cmd.1)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let make_state = |dir: std::path::PathBuf| -> Shared { Arc::new(AppState {
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("http client"),
        providers: providers::registry(),
        snapshots: RwLock::new(vec![]),
        store: RwLock::new(Store::load(dir)),
        wake: Notify::new(),
        billing_ops: Default::default(),
    }) };

    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            get_snapshots,
            refresh,
            start_login,
            finish_login,
            logout,
            set_billing_token,
            set_provider_order,
            set_account_order,
            set_account_hidden,
            get_tray_display,
            set_tray_display,
            get_refresh_secs,
            set_refresh_secs,
            open_url,
            quit
        ])
        .setup(move |app| {
            let state = make_state(app.path().app_config_dir()?);
            app.manage(state.clone());
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let win = app.get_webview_window("main").unwrap();
            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
                let _ = apply_vibrancy(
                    &win,
                    NSVisualEffectMaterial::Popover,
                    Some(NSVisualEffectState::Active),
                    Some(14.0),
                );
            }
            #[cfg(target_os = "windows")]
            let _ = window_vibrancy::apply_mica(&win, None);

            TrayIconBuilder::with_id(TRAY_ID)
                .icon(tauri::include_image!("icons/tray.png"))
                .icon_as_template(true)
                .tooltip("Agent Usage")
                .on_tray_icon_event(|tray, event| {
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_panel(tray.app_handle());
                    }
                })
                .build(app)?;

            // Background refresh loop: sleeps until the interval elapses or someone pokes `wake`.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                migrate_legacy(&state).await;
                loop {
                    refresh_all(&handle, &state).await;
                    let secs = state.store.read().await.config.refresh_secs;
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(secs)) => {}
                        _ = state.wake.notified() => {}
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|win, event| {
            if let WindowEvent::Focused(false) = event {
                *LAST_BLUR_HIDE.lock().unwrap() = Some(std::time::Instant::now());
                let _ = win.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
