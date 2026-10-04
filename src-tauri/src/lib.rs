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
use tauri_plugin_opener::OpenerExt;
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
                        Err(e) => s.error = Some(format!("Couldn't read the token from the system credential store: {e}")),
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
/// `order` lists only the providers the user could drag (those shown in the overview).
/// They're placed into the slots they already occupy in the full order, so hidden and
/// unconnected providers keep their positions.
async fn set_provider_order(app: AppHandle, state: State<'_, Shared>, order: Vec<String>) -> Result<(), String> {
    {
        let mut st = state.store.write().await;
        let mut full: Vec<String> =
            ordered_providers(&state, &st.config.provider_order).iter().map(|p| p.id().to_string()).collect();
        let mut next = order.iter();
        for slot in full.iter_mut().filter(|id| order.contains(id)) {
            *slot = next.next().unwrap().clone();
        }
        st.config.provider_order = full;
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
    let title = tray_title(snaps, display);
    // Windows tray icons have no title; expose the same preference in the tooltip.
    #[cfg(target_os = "macos")]
    let _ = tray.set_title(title.as_deref());
    let _ = tray.set_tooltip(Some(tray_tooltip(title.as_deref())));
}

fn tray_tooltip(title: Option<&str>) -> String {
    match title {
        Some(title) => format!("Agent Usage — {title} remaining"),
        None => "Agent Usage".into(),
    }
}

fn constrain_panel_position(
    position: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
    work_area: &tauri::PhysicalRect<i32, u32>,
) -> tauri::PhysicalPosition<i32> {
    // A panel can briefly exceed the work area while the frontend adapts to a
    // smaller monitor. Anchor that axis at the work area's origin until it fits.
    let clamp_axis = |value: i32, origin: i32, available: u32, required: u32| {
        let min = i64::from(origin);
        let max = min + i64::from(available.saturating_sub(required));
        i64::from(value).clamp(min, max) as i32
    };
    tauri::PhysicalPosition::new(
        clamp_axis(position.x, work_area.position.x, work_area.size.width, size.width),
        clamp_axis(position.y, work_area.position.y, work_area.size.height, size.height),
    )
}

fn position_panel(win: &tauri::Window) -> tauri::Result<()> {
    // Use the tray's monitor even when the panel was last opened on another one.
    let tray_monitor = win.app_handle().tray_by_id(TRAY_ID)
        .and_then(|tray| tray.rect().ok().flatten())
        .and_then(|rect| {
            // Tray rectangles use physical pixels, like window geometry and work_area.
            let position = rect.position.to_physical::<f64>(1.0);
            win.monitor_from_point(position.x, position.y).ok().flatten()
        });
    // Before the first tray event (e.g. a second launch), its position is unknown.
    if win.move_window_constrained(Position::TrayCenter).is_err() {
        win.center()?;
    }
    let monitor = match tray_monitor {
        Some(monitor) => Some(monitor),
        None => win.current_monitor()?,
    };
    if let Some(monitor) = monitor {
        // The positioner only constrains to the full screen, including taskbars.
        let position = win.outer_position()?;
        let constrained = constrain_panel_position(position, win.outer_size()?, monitor.work_area());
        if constrained != position {
            win.set_position(constrained)?;
        }
    }
    Ok(())
}

fn show_panel(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else { return };
    // show() preserves the minimized state, and Windows won't focus a minimized window.
    let _ = win.unminimize();
    let _ = position_panel(&win.as_ref().window());
    let _ = win.show();
    let _ = win.set_focus();
    let _ = app.emit("panel-shown", ());
    if let Some(state) = app.try_state::<Shared>() {
        state.wake.notify_one();
    }
}

#[cfg(target_os = "windows")]
fn set_panel_background(win: &tauri::WebviewWindow, theme: tauri::Theme) {
    // Mica only works on Windows 11. An opaque, theme-aware WebView background
    // also keeps the panel readable on Windows 10 and when effects are disabled.
    let color = match theme {
        tauri::Theme::Dark => tauri::window::Color(28, 28, 30, 255),
        _ => tauri::window::Color(245, 245, 247, 255),
    };
    let _ = win.set_background_color(Some(color));
}

fn toggle_panel(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else { return };
    let just_hidden = LAST_BLUR_HIDE
        .lock()
        .unwrap()
        .take()
        .is_some_and(|t| t.elapsed() < Duration::from_millis(300));
    if win.is_minimized().unwrap_or(false) {
        show_panel(app);
    } else if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
    } else if !just_hidden {
        show_panel(app);
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
        Some(e) => Err(format!("Signed out, but the token couldn't be removed from the system credential store: {e}")),
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
fn get_platform() -> &'static str {
    std::env::consts::OS
}

fn web_url(url: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(url).map_err(|e| e.to_string())?;
    if !matches!(url.scheme(), "https" | "http") || url.host_str().is_none() {
        return Err("Only HTTP and HTTPS links can be opened".into());
    }
    Ok(url)
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    let url = web_url(&url)?;
    app.opener().open_url(url.as_str(), None::<&str>).map_err(|e| e.to_string())
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
        // Register first so launching from Start or a shortcut reuses this process.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show_panel(app)))
        .plugin(tauri_plugin_opener::init())
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
            get_platform,
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
            set_panel_background(&win, win.theme().unwrap_or(tauri::Theme::Light));

            let tray = TrayIconBuilder::with_id(TRAY_ID)
                .tooltip("Agent Usage")
                .show_menu_on_left_click(false);
            #[cfg(target_os = "macos")]
            let tray = tray.icon(tauri::include_image!("icons/tray.png")).icon_as_template(true);
            #[cfg(not(target_os = "macos"))]
            let tray = tray.icon(tauri::include_image!("icons/32x32.png"));
            #[cfg(target_os = "windows")]
            let tray = {
                use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
                let open = MenuItem::with_id(app, "open", "Open Agent Usage", true, None::<&str>)?;
                let refresh = MenuItem::with_id(app, "refresh", "Refresh", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&open, &refresh, &PredefinedMenuItem::separator(app)?, &quit])?;
                tray.menu(&menu).on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_panel(app),
                    "refresh" => app.state::<Shared>().wake.notify_one(),
                    "quit" => app.exit(0),
                    _ => {}
                })
            };
            tray
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
            match event {
                WindowEvent::Focused(false) => {
                    *LAST_BLUR_HIDE.lock().unwrap() = Some(std::time::Instant::now());
                    let _ = win.hide();
                }
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = win.hide();
                }
                WindowEvent::Resized(_) if win.is_visible().unwrap_or(false) && !win.is_minimized().unwrap_or(false) => {
                    // Keep the bottom-anchored Windows panel on screen as its content grows.
                    let _ = position_panel(win);
                }
                #[cfg(target_os = "windows")]
                WindowEvent::ThemeChanged(theme) => {
                    if let Some(webview) = win.app_handle().get_webview_window("main") {
                        set_panel_background(&webview, *theme);
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use providers::UsageWindow;

    #[test]
    fn tray_overflow_panel_stays_above_bottom_taskbar() {
        // TrayCenter can place the panel below an overflow icon, then clamp it to
        // y=480 on a 1080px screen. The bottom 40px belong to the taskbar.
        let work_area = tauri::PhysicalRect {
            position: tauri::PhysicalPosition::new(0, 0),
            size: tauri::PhysicalSize::new(1920, 1040),
        };
        let position = constrain_panel_position(
            tauri::PhysicalPosition::new(1510, 480),
            tauri::PhysicalSize::new(360, 600),
            &work_area,
        );
        assert_eq!(position, tauri::PhysicalPosition::new(1510, 440));
    }

    #[test]
    fn panel_respects_top_and_side_taskbars() {
        for (origin, available, desired, expected) in [
            ((0, 40), (1920, 1040), (500, 0), (500, 40)),
            ((80, 0), (1840, 1080), (0, 400), (80, 400)),
            ((0, 0), (1840, 1080), (1560, 400), (1480, 400)),
        ] {
            let work_area = tauri::PhysicalRect {
                position: tauri::PhysicalPosition::from(origin),
                size: tauri::PhysicalSize::from(available),
            };
            assert_eq!(
                constrain_panel_position(desired.into(), (360, 600).into(), &work_area),
                tauri::PhysicalPosition::from(expected),
            );
        }
    }

    #[test]
    fn panel_uses_physical_pixels_and_signed_monitor_coordinates() {
        // A secondary monitor above/left of the primary, at 150% scale.
        let work_area = tauri::PhysicalRect {
            position: tauri::PhysicalPosition::new(-1920, -1080),
            size: tauri::PhysicalSize::new(1920, 1040),
        };
        let size = tauri::PhysicalSize::new(540, 900);
        assert_eq!(
            constrain_panel_position((-200, -900).into(), size, &work_area),
            tauri::PhysicalPosition::new(-540, -940),
        );
        let inside = tauri::PhysicalPosition::new(-1000, -1000);
        assert_eq!(constrain_panel_position(inside, size, &work_area), inside);
    }

    #[test]
    fn oversized_panel_anchors_inside_work_area_until_resized() {
        let work_area = tauri::PhysicalRect {
            position: tauri::PhysicalPosition::new(0, 40),
            size: tauri::PhysicalSize::new(320, 440),
        };
        assert_eq!(
            constrain_panel_position((-40, -120).into(), (360, 600).into(), &work_area),
            work_area.position,
        );
    }

    fn snapshot(account: &str, used: f64) -> UsageSnapshot {
        let mut snap = UsageSnapshot::empty(&providers::copilot::Copilot, Some(account));
        snap.windows.push(UsageWindow {
            label: "Monthly limit".into(),
            used,
            limit: Some(100.0),
            resets_at: None,
        });
        snap
    }

    #[test]
    fn lowest_remaining_ignores_hidden_and_unlimited_accounts() {
        let visible = snapshot("visible", 25.0);
        let mut hidden = snapshot("hidden", 99.0);
        hidden.hidden = true;
        let mut unlimited = snapshot("unlimited", 200.0);
        unlimited.windows[0].limit = None;
        assert_eq!(tray_title(&[visible, hidden, unlimited], &TrayDisplay::Lowest).as_deref(), Some("75%"));
    }

    #[test]
    fn pinned_quota_matches_provider_and_account_even_when_hidden() {
        let mut other_provider = snapshot("alice", 99.0);
        other_provider.provider_id = "codex".into();
        let mut pinned = snapshot("alice", 25.0);
        pinned.hidden = true;
        let display = TrayDisplay::Pinned { provider: "copilot".into(), account: "alice".into() };
        assert_eq!(tray_title(&[other_provider, snapshot("bob", 90.0), pinned], &display).as_deref(), Some("75%"));
        assert_eq!(tray_title(&[], &display), None);
    }

    #[test]
    fn remaining_percentage_is_floored_and_clamped() {
        for (used, expected) in [(25.1, "74%"), (110.0, "0%"), (-1.0, "100%")] {
            assert_eq!(tray_title(&[snapshot("alice", used)], &TrayDisplay::Lowest).as_deref(), Some(expected));
        }
    }

    #[test]
    fn tooltip_clears_quota_for_icon_only_or_missing_data() {
        let snaps = [snapshot("alice", 25.0)];
        let title = tray_title(&snaps, &TrayDisplay::Lowest);
        assert_eq!(tray_tooltip(title.as_deref()), "Agent Usage — 75% remaining");
        assert_eq!(tray_tooltip(tray_title(&snaps, &TrayDisplay::IconOnly).as_deref()), "Agent Usage");
        assert_eq!(tray_tooltip(tray_title(&[], &TrayDisplay::Lowest).as_deref()), "Agent Usage");
    }

    #[test]
    fn browser_url_preserves_billing_query_parameters() {
        let url = "https://github.com/settings/tokens/new?scopes=user&description=Agent%20Usage%20billing";
        assert_eq!(web_url(url).unwrap().as_str(), url);
        assert!(web_url("http://localhost:8080/login?code=abc&state=def").is_ok());
    }

    #[test]
    fn browser_url_rejects_files_commands_and_non_web_schemes() {
        for url in ["file:///C:/Windows/system32/cmd.exe", "javascript:alert(1)", "cmd /C start", "ms-settings:privacy", ""] {
            assert!(web_url(url).is_err(), "accepted {url}");
        }
    }
}
