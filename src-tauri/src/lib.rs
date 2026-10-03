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
}

type Shared = Arc<AppState>;

impl AppState {
    fn provider(&self, id: &str) -> Result<&dyn Provider, String> {
        self.providers
            .iter()
            .find(|p| p.id() == id)
            .map(|p| p.as_ref())
            .ok_or_else(|| format!("unknown provider {id}"))
    }
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
            let snap = match store::get_secret(&a.provider, &a.id) {
                Some(secret) => p.fetch(&state.http, &a.id, &secret).await,
                None => {
                    let mut s = UsageSnapshot::empty(p.as_ref(), Some(&a.id));
                    s.needs_auth = true;
                    s
                }
            };
            snaps.push(snap);
        }
    }
    let display = state.store.read().await.config.tray_display.clone();
    update_tray(app, &snaps, &display);
    *state.snapshots.write().await = snaps.clone();
    let _ = app.emit("usage-updated", snaps);
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
        TrayDisplay::Lowest => snaps.iter().filter_map(|s| s.min_remaining()).reduce(f64::min),
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
    store::delete_secret(&provider, &account)?;
    if provider == "copilot" {
        store::delete_secret(providers::copilot::BILLING_KEY, &account)?;
    }
    {
        let mut st = state.store.write().await;
        st.config.accounts.retain(|a| !(a.provider == provider && a.id == account));
        if matches!(&st.config.tray_display, TrayDisplay::Pinned { provider: p, account: a } if *p == provider && *a == account) {
            st.config.tray_display = TrayDisplay::Lowest;
        }
        st.save()?;
    }
    state.wake.notify_one();
    Ok(())
}

/// Saves (or with an empty token, removes) a Copilot account's billing PAT after verifying it.
#[tauri::command]
async fn set_billing_token(state: State<'_, Shared>, account: String, token: String) -> Result<(), String> {
    let token = token.trim();
    if token.is_empty() {
        store::delete_secret(providers::copilot::BILLING_KEY, &account)?;
    } else {
        providers::copilot::verify_billing_token(&state.http, &account, token).await?;
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
