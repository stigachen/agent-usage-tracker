mod providers;

use providers::{DeviceCode, Provider, UsageSnapshot};
use std::sync::Arc;
use std::time::Duration;
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WindowEvent,
};
use tauri_plugin_positioner::{Position, WindowExt};
use tokio::sync::{Notify, RwLock};

const TRAY_ID: &str = "main";

struct AppState {
    http: reqwest::Client,
    providers: Vec<Box<dyn Provider>>,
    snapshots: RwLock<Vec<UsageSnapshot>>,
    interval_secs: RwLock<u64>,
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
    let futs = state.providers.iter().map(|p| p.fetch(&state.http));
    let snaps = fetch_each(futs).await;
    update_tray(app, &snaps);
    *state.snapshots.write().await = snaps.clone();
    let _ = app.emit("usage-updated", snaps);
}

// Fetches sequentially; fine for a handful of providers and avoids the `futures` crate.
async fn fetch_each<F: std::future::Future>(futs: impl Iterator<Item = F>) -> Vec<F::Output> {
    let handles: Vec<_> = futs.collect();
    let mut out = Vec::with_capacity(handles.len());
    for f in handles {
        out.push(f.await);
    }
    out
}

fn update_tray(app: &AppHandle, snaps: &[UsageSnapshot]) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else { return };
    let title = snaps
        .iter()
        .filter_map(|s| s.max_ratio())
        .reduce(f64::max)
        .map(|r| format!("{:.0}%", r * 100.0));
    let _ = tray.set_title(title.as_deref());
}

fn toggle_panel(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else { return };
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
    } else {
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
    state.provider(&provider)?.finish_login(&state.http, code).await?;
    state.wake.notify_one();
    Ok(())
}

#[tauri::command]
fn logout(state: State<'_, Shared>, provider: String) -> Result<(), String> {
    state.provider(&provider)?.logout()?;
    state.wake.notify_one();
    Ok(())
}

#[tauri::command]
async fn set_interval(state: State<'_, Shared>, secs: u64) -> Result<(), ()> {
    *state.interval_secs.write().await = secs.clamp(60, 3600);
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
    let state: Shared = Arc::new(AppState {
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("http client"),
        providers: providers::registry(),
        snapshots: RwLock::new(vec![]),
        interval_secs: RwLock::new(600),
        wake: Notify::new(),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            get_snapshots,
            refresh,
            start_login,
            finish_login,
            logout,
            set_interval,
            open_url,
            quit
        ])
        .setup(move |app| {
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
                loop {
                    refresh_all(&handle, &state).await;
                    let secs = *state.interval_secs.read().await;
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
                let _ = win.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
