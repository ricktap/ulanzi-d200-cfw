pub mod apps;
pub mod bind;
#[cfg(test)]
mod guide_tests;
pub mod cards;
pub mod config;
pub mod device;
pub mod engine;
pub mod icons;
pub mod ide;
pub mod media;
pub mod migrate;
pub mod teams;
pub mod mqtt;
pub mod platform;
pub mod registry;
pub mod render;
pub mod screensaver;

use config::{Config, KeyEvent};
use engine::{Engine, Handle, Ui};
use render::Notification;
use serde_json::Value;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

/// The device firmware, so the app can start it on a D200 without the shell scripts.
pub static FIRMWARE: &[u8] = include_bytes!("../../../bin/d200fw");

type H<'a> = State<'a, Handle>;

/// Run a blocking engine request off the async runtime.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_state(h: H<'_>) -> Result<Value, String> {
    let h = h.inner().clone();
    blocking(move || h.ask(Ui::Snapshot)).await
}

#[tauri::command]
async fn save_config(h: H<'_>, config: Value) -> Result<Value, String> {
    let (config, _) = Config::from_value(config)?;
    let h = h.inner().clone();
    blocking(move || h.ask(|r| Ui::Save(Box::new(config), r))?).await
}

#[tauri::command]
async fn import_config(h: H<'_>, json: String) -> Result<Value, String> {
    let cfg = Config::parse(&json).map_err(|e| format!("not a valid deck config: {e}"))?;
    let h = h.inner().clone();
    blocking(move || h.ask(|r| Ui::Save(Box::new(cfg), r))?).await
}

#[tauri::command]
fn set_view(h: H<'_>, profile: usize, page: String, remote: bool) {
    h.send(Ui::View { profile, page, remote });
}

#[tauri::command]
async fn preview(h: H<'_>, profile: usize, page: String, remote: bool, key: usize) -> Result<String, String> {
    let h = h.inner().clone();
    blocking(move || h.ask(|reply| Ui::Preview { profile, page, remote, key, reply })?).await
}

#[tauri::command]
fn trigger(h: H<'_>, key: usize, event: KeyEvent) {
    h.send(Ui::Trigger { key, event });
}

#[tauri::command]
fn notify(h: H<'_>, notification: Notification) {
    h.send(Ui::Notify(notification));
}

#[tauri::command]
async fn start_firmware(h: H<'_>) -> Result<String, String> {
    let h = h.inner().clone();
    blocking(move || h.start_firmware()).await
}

#[tauri::command]
async fn running_apps() -> Result<Vec<platform::AppInfo>, String> {
    blocking(|| Ok(platform::running_apps())).await
}

#[tauri::command]
async fn geocode(query: String) -> Result<Vec<Value>, String> {
    blocking(move || apps::geocode(&query)).await
}

/// Start the Microsoft 365 device-code sign-in for Teams presence. Returns the
/// code to show; completion arrives as the "teams-signin" event.
#[tauri::command]
async fn teams_sign_in(app: AppHandle, h: H<'_>, client_id: String, tenant: String) -> Result<Value, String> {
    let h = h.inner().clone();
    blocking(move || {
        let dc = teams::start_device_code(&client_id, &tenant)?;
        let out = serde_json::json!({ "user_code": dc.user_code, "verification_uri": dc.verification_uri });
        let _ = platform::open(&dc.verification_uri, std::path::Path::new("/"));
        std::thread::spawn(move || {
            let r = teams::wait_device_code(&client_id, &tenant, &dc);
            match r {
                Ok(t) => {
                    let account = t.account.clone();
                    h.send(Ui::GraphTokens(Some(t)));
                    let _ = app.emit("teams-signin", serde_json::json!({ "ok": true, "account": account }));
                }
                Err(e) => {
                    let _ = app.emit("teams-signin", serde_json::json!({ "ok": false, "error": e }));
                }
            }
        });
        Ok(out)
    })
    .await
}

/// Everything packages contribute: plugins, interfaces, signals, actions, apps, icons.
#[tauri::command]
fn catalog() -> Value {
    registry::builtin().catalog()
}

/// Values for a `select` field with `{"source": ...}`.
#[tauri::command]
async fn options(h: H<'_>, source: String, profile: usize) -> Result<Vec<Value>, String> {
    let h = h.inner().clone();
    blocking(move || h.ask(|reply| Ui::Options { source, profile, reply })).await
}

/// Current value of every signal.
#[tauri::command]
async fn signals(h: H<'_>) -> Result<Value, String> {
    let h = h.inner().clone();
    blocking(move || h.ask(Ui::Signals)).await
}

/// `icon` drawn with the live values of the app on a key (for the icon picker).
#[tauri::command]
async fn preview_icon(h: H<'_>, icon: String, profile: usize, page: String, key: usize) -> Result<String, String> {
    let h = h.inner().clone();
    blocking(move || h.ask(|reply| Ui::PreviewIcon { icon, profile, page, key, reply })?).await
}

/// Icon names and groups for the icon picker.
#[tauri::command]
fn icon_set() -> Value {
    serde_json::from_str::<Value>(icons::MANIFEST).map(|m| m["icons"].clone()).unwrap_or_default()
}

#[tauri::command]
fn screensaver(h: H<'_>, on: bool) {
    h.send(Ui::Screensaver(on));
}

#[tauri::command]
fn teams_sign_out(h: H<'_>) {
    h.send(Ui::GraphTokens(None));
}

#[tauri::command]
fn open_config_dir(h: H<'_>) -> Result<(), String> {
    platform::open(&h.config_dir().display().to_string(), &h.config_dir()).map_err(|e| e.to_string())
}

#[tauri::command]
fn autostart_get(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn autostart_set(app: AppHandle, on: bool) -> Result<bool, String> {
    let al = app.autolaunch();
    if on { al.enable() } else { al.disable() }.map_err(|e| e.to_string())?;
    Ok(al.is_enabled().unwrap_or(false))
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .setup(|app| {
            let path = app.path().app_config_dir()?.join("deck.json");
            let handle = app.handle().clone();
            let emit: engine::Emit = Box::new(move |name, payload| {
                let _ = handle.emit(name, payload);
            });
            app.manage(Engine::spawn(path, emit, FIRMWARE));

            // Menu-bar icon: the deck keeps running when the window is closed.
            let show = MenuItem::with_id(app, "show", "Show D200 Deck", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            TrayIconBuilder::with_id("tray")
                .icon(app.default_window_icon().cloned().expect("app icon"))
                .tooltip("D200 Deck")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, ev| match ev.id.as_ref() {
                    "show" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, ev| {
                    if let TrayIconEvent::DoubleClick { button: MouseButton::Left, .. } = ev {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;

            if std::env::args().any(|a| a == "--hidden") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            catalog,
            options,
            signals,
            preview_icon,
            save_config,
            import_config,
            set_view,
            preview,
            trigger,
            notify,
            start_firmware,
            running_apps,
            geocode,
            teams_sign_in,
            teams_sign_out,
            screensaver,
            icon_set,
            open_config_dir,
            autostart_get,
            autostart_set
        ])
        .build(tauri::generate_context!())
        .expect("error while building the app");

    app.run(|app, ev| {
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen { .. } = ev {
            show_main(app);
        }
        let _ = (app, ev);
    });
}
