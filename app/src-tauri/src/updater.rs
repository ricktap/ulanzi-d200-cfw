//! Updates from the GitHub releases (tauri-plugin-updater).
//!
//! To stay far below GitHub's rate limits, the app first asks the releases API for the
//! latest release with the ETag of its last answer (`If-None-Match`). A `304 Not
//! Modified` costs nothing against the limit. Only when that release is newer than the
//! running app does the updater fetch `latest.json`, and the package itself only when
//! the user installs it.
//!
//! CI signs the update with the key whose public half is in tauri.conf.json
//! (`plugins.updater.pubkey`) and publishes `latest.json` with every release.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

const LATEST: &str = "https://api.github.com/repos/ricktap/ulanzi-d200-cfw/releases/latest";
const EVERY: Duration = Duration::from_secs(6 * 3600);

/// The last answer of the releases API, kept next to deck.json.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Seen {
    pub etag: Option<String>,
    pub tag: Option<String>,
}

fn state_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("update-check.json"))
}

fn load(app: &AppHandle) -> Seen {
    state_path(app).and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn store(app: &AppHandle, s: &Seen) {
    if let Some(p) = state_path(app) {
        let _ = crate::config::write_private(&p, &serde_json::to_string(s).unwrap_or_default());
    }
}

/// Ask for the latest release tag, conditionally. Returns the tag (from the answer, or
/// the stored one on 304) and whether it was a fresh answer.
fn latest_tag(seen: &Seen) -> Result<(Seen, bool), String> {
    let mut req = ureq::get(LATEST)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "D200-Deck-updater")
        .timeout(Duration::from_secs(15));
    if let Some(e) = &seen.etag {
        req = req.set("If-None-Match", e);
    }
    match req.call() {
        Ok(r) if r.status() == 304 => Ok((seen.clone(), false)),
        Ok(r) => {
            let etag = r.header("ETag").map(str::to_string);
            let v: Value = r.into_json().map_err(|e| e.to_string())?;
            Ok((Seen { etag, tag: v["tag_name"].as_str().map(str::to_string) }, true))
        }
        // ureq reports 304 as a status "error" in some versions
        Err(ureq::Error::Status(304, _)) => Ok((seen.clone(), false)),
        Err(ureq::Error::Status(404, _)) => Ok((Seen::default(), true)), // no release yet
        Err(e) => Err(e.to_string()),
    }
}

/// `a` is a newer semver than `b` ("v1.2.3", pre-releases sort before their release).
pub fn newer(a: &str, b: &str) -> bool {
    fn parse(v: &str) -> (Vec<u64>, Option<String>) {
        let v = v.trim().trim_start_matches('v');
        let (core, pre) = match v.split_once('-') {
            Some((c, p)) => (c, Some(p.to_string())),
            None => (v.split('+').next().unwrap_or(v), None),
        };
        (core.split('.').map(|n| n.parse().unwrap_or(0)).collect(), pre)
    }
    let (ca, pa) = parse(a);
    let (cb, pb) = parse(b);
    match ca.cmp(&cb) {
        std::cmp::Ordering::Equal => matches!((&pa, &pb), (None, Some(_))) || matches!((&pa, &pb), (Some(x), Some(y)) if x > y),
        o => o == std::cmp::Ordering::Greater,
    }
}

/// Serialises checks (background and the Settings button share the stored ETag).
static LOCK: Mutex<()> = Mutex::new(());

/// The newer release tag, if any, asking GitHub only conditionally.
fn newer_release(app: &AppHandle) -> Result<Option<String>, String> {
    let _g = LOCK.lock().unwrap();
    let seen = load(app);
    let (now, fresh) = latest_tag(&seen)?;
    if fresh {
        store(app, &now);
    }
    let current = app.package_info().version.to_string();
    Ok(now.tag.filter(|t| newer(t, &current)))
}

async fn fetch(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>, String> {
    app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())
}

fn describe(u: &tauri_plugin_updater::Update) -> Value {
    json!({ "version": u.version, "current": u.current_version, "notes": u.body })
}

/// The newer version, if there is one: {version, current, notes}.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<Value>, String> {
    let a = app.clone();
    let tag = tauri::async_runtime::spawn_blocking(move || newer_release(&a)).await.map_err(|e| e.to_string())??;
    if tag.is_none() {
        return Ok(None);
    }
    Ok(fetch(&app).await?.as_ref().map(describe))
}

/// Download, install and restart.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = fetch(&app).await?.ok_or("D200 Deck is up to date")?;
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    app.restart()
}

#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Look for updates in the background: shortly after start, then every six hours.
/// The UI hears of each new version once (`update` event).
pub fn watch(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(15));
        let mut told: Option<String> = None;
        loop {
            match newer_release(&app) {
                Ok(Some(tag)) if told.as_deref() != Some(tag.as_str()) => {
                    if let Ok(Some(u)) = tauri::async_runtime::block_on(fetch(&app)) {
                        let _ = app.emit("update", describe(&u));
                        told = Some(tag);
                    }
                }
                Ok(_) => {}
                Err(e) => eprintln!("update check: {e}"),
            }
            std::thread::sleep(EVERY);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::newer;

    #[test]
    fn versions() {
        assert!(newer("v0.4.0", "0.3.0"));
        assert!(newer("v1.0.0", "0.9.9"));
        assert!(!newer("v0.3.0", "0.3.0"));
        assert!(!newer("v0.2.9", "0.3.0"));
        assert!(newer("v0.3.0", "0.3.0-rc.1"));
        assert!(!newer("v0.3.0-rc.1", "0.3.0"));
        assert!(newer("v0.3.0-rc.2", "0.3.0-rc.1"));
    }
}
