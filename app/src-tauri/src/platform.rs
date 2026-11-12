//! OS integration: frontmost app, running apps, system mute, open, shortcuts,
//! shell commands. macOS is the primary target; Windows and Linux get what is
//! easy to do there (see the notes on each function).

use serde::Serialize;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppInfo {
    /// Display name ("Safari").
    pub name: String,
    /// Stable id: bundle id on macOS ("com.apple.Safari"), exe path on Windows.
    pub id: String,
}

impl AppInfo {
    /// Does a profile binding entry ("com.apple.Safari", "Safari", "chrome.exe") match?
    pub fn matches(&self, entry: &str) -> bool {
        let e = entry.trim();
        if e.is_empty() {
            return false;
        }
        let file = self.id.rsplit(['/', '\\']).next().unwrap_or("");
        e.eq_ignore_ascii_case(&self.id)
            || e.eq_ignore_ascii_case(&self.name)
            || e.eq_ignore_ascii_case(file)
            || e.eq_ignore_ascii_case(file.trim_end_matches(".exe"))
    }
}

// ------------------------------------------------------------------ frontmost / running apps

#[cfg(target_os = "macos")]
mod mac {
    use super::AppInfo;
    use objc2::rc::autoreleasepool;
    use objc2_app_kit::{NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace};

    fn info(app: &NSRunningApplication) -> Option<AppInfo> {
        let name = app.localizedName().map(|s| s.to_string()).unwrap_or_default();
        let id = app.bundleIdentifier().map(|s| s.to_string()).unwrap_or_else(|| name.clone());
        (!name.is_empty() || !id.is_empty()).then_some(AppInfo { name, id })
    }

    pub fn frontmost() -> Option<AppInfo> {
        autoreleasepool(|_| {
            let ws = NSWorkspace::sharedWorkspace();
            ws.frontmostApplication().and_then(|a| info(&a))
        })
    }

    /// Is an app with one of these bundle ids running? Native and cheap (no AppleScript).
    pub fn any_running(bundle_ids: &[&str]) -> bool {
        autoreleasepool(|_| {
            let ws = NSWorkspace::sharedWorkspace();
            ws.runningApplications()
                .iter()
                .any(|a| a.bundleIdentifier().is_some_and(|b| bundle_ids.iter().any(|id| b.to_string() == *id)))
        })
    }

    /// Default output device muted, from CoreAudio. None when the device has no mute
    /// control (then the caller asks AppleScript).
    pub fn output_muted() -> Option<bool> {
        use std::ffi::c_void;
        #[repr(C)]
        struct Address {
            selector: u32,
            scope: u32,
            element: u32,
        }
        #[link(name = "CoreAudio", kind = "framework")]
        extern "C" {
            fn AudioObjectGetPropertyData(id: u32, addr: *const Address, qsize: u32, qdata: *const c_void, size: *mut u32, data: *mut c_void) -> i32;
        }
        let cc = |s: &[u8; 4]| u32::from_be_bytes(*s);
        const SYSTEM_OBJECT: u32 = 1;
        let mut device: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let addr = Address { selector: cc(b"dOut"), scope: cc(b"glob"), element: 0 };
        // SAFETY: plain C call with a valid address struct and an out-buffer of `size` bytes.
        let st = unsafe { AudioObjectGetPropertyData(SYSTEM_OBJECT, &addr, 0, std::ptr::null(), &mut size, &mut device as *mut u32 as *mut c_void) };
        if st != 0 || device == 0 {
            return None;
        }
        let mut muted: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let addr = Address { selector: cc(b"mute"), scope: cc(b"outp"), element: 0 };
        // SAFETY: as above.
        let st = unsafe { AudioObjectGetPropertyData(device, &addr, 0, std::ptr::null(), &mut size, &mut muted as *mut u32 as *mut c_void) };
        (st == 0).then_some(muted != 0)
    }

    pub fn running() -> Vec<AppInfo> {
        autoreleasepool(|_| {
            let ws = NSWorkspace::sharedWorkspace();
            ws.runningApplications()
                .iter()
                .filter(|a| a.activationPolicy() == NSApplicationActivationPolicy::Regular)
                .filter_map(|a| info(&a))
                .collect()
        })
    }
}

#[cfg(windows)]
mod win {
    use super::AppInfo;
    use windows_sys::Win32::Foundation::{CloseHandle, MAX_PATH};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    pub fn frontmost() -> Option<AppInfo> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return None;
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == 0 {
                return None;
            }
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut buf = [0u16; MAX_PATH as usize * 2];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
            CloseHandle(h);
            if ok == 0 {
                return None;
            }
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            let name = path.rsplit('\\').next().unwrap_or(&path).trim_end_matches(".exe").to_string();
            Some(AppInfo { name, id: path })
        }
    }
}

/// The app in front. macOS and Windows; `None` elsewhere.
/// Can this OS tell the frontmost app? Then scans for games and debuggers run only when
/// the front app changes or is the one they look for.
pub fn front_app_supported() -> bool {
    cfg!(any(target_os = "macos", windows))
}

/// Is one of these apps running? (bundle ids on macOS). Elsewhere: assume yes.
pub fn any_running(bundle_ids: &[&str]) -> bool {
    #[cfg(target_os = "macos")]
    return mac::any_running(bundle_ids);
    #[allow(unreachable_code)]
    {
        let _ = bundle_ids;
        true
    }
}

pub fn frontmost_app() -> Option<AppInfo> {
    #[cfg(target_os = "macos")]
    return mac::frontmost();
    #[cfg(windows)]
    return win::frontmost();
    #[allow(unreachable_code)]
    None
}

/// Apps with a window, for the profile binding picker (macOS only; elsewhere
/// the current frontmost app is offered).
pub fn running_apps() -> Vec<AppInfo> {
    #[cfg(target_os = "macos")]
    {
        let mut v = mac::running();
        v.sort_by_key(|a| a.name.to_lowercase());
        v.dedup_by(|a, b| a.id == b.id);
        v
    }
    #[cfg(not(target_os = "macos"))]
    {
        frontmost_app().into_iter().collect()
    }
}

// ------------------------------------------------------------------ commands

fn quiet(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

fn output(cmd: &mut Command) -> Option<String> {
    let out = quiet(cmd).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Start a shell command in the background (in `dir`).
pub fn run_shell(cmd: &str, dir: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let mut c = {
        let mut c = Command::new("cmd");
        c.args(["/C", cmd]);
        c
    };
    #[cfg(not(windows))]
    let mut c = {
        let mut c = Command::new("/bin/sh");
        c.args(["-c", cmd]);
        // Apps started from Finder get a minimal PATH; add the usual tool folders.
        let path = std::env::var("PATH").unwrap_or_default();
        c.env("PATH", format!("/opt/homebrew/bin:/usr/local/bin:{path}:/usr/bin:/bin:/usr/sbin:/sbin"));
        c
    };
    if dir.is_dir() {
        c.current_dir(dir);
    }
    let mut child = quiet(&mut c).spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Open an app by name, a file/folder, or a URL.
pub fn open(target: &str, dir: &Path) -> std::io::Result<()> {
    let t = target.trim();
    let looks_like_path = t.contains("://") || t.starts_with('/') || t.starts_with('~') || t.contains('\\');
    #[cfg(target_os = "macos")]
    let cmd = if looks_like_path {
        format!("open {}", sh_quote(&expand_home(t)))
    } else {
        format!("open -a {}", sh_quote(t))
    };
    #[cfg(windows)]
    let cmd = {
        let _ = looks_like_path;
        format!("start \"\" \"{t}\"")
    };
    #[cfg(all(not(target_os = "macos"), not(windows)))]
    let cmd = if looks_like_path { format!("xdg-open {}", sh_quote(&expand_home(t))) } else { sh_quote(t) };
    run_shell(&cmd, dir)
}

#[cfg(not(windows))]
fn expand_home(p: &str) -> String {
    match (p.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(h)) => format!("{h}/{rest}"),
        _ => p.to_string(),
    }
}

#[cfg(not(windows))]
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Send a keyboard shortcut like "cmd+shift+4" to the frontmost app.
/// macOS only (System Events); the app needs the Accessibility permission.
pub fn send_keys(combo: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let script = applescript_for_combo(combo)?;
        let ok = quiet(Command::new("osascript").args(["-e", &script]))
            .output()
            .map_err(|e| e.to_string())?;
        if ok.status.success() {
            Ok(())
        } else {
            Err(format!(
                "shortcut failed (allow D200 Deck under System Settings → Privacy & Security → Accessibility): {}",
                String::from_utf8_lossy(&ok.stderr).trim()
            ))
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = combo;
        Err("keyboard shortcuts are only supported on macOS so far".into())
    }
}

/// AppleScript for a shortcut. Public for tests.
pub fn applescript_for_combo(combo: &str) -> Result<String, String> {
    let mut mods = vec![];
    let mut key: Option<String> = None;
    for part in combo.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match part.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "meta" | "super" => mods.push("command down"),
            "ctrl" | "control" => mods.push("control down"),
            "alt" | "opt" | "option" => mods.push("option down"),
            "shift" => mods.push("shift down"),
            _ => key = Some(part.to_string()),
        }
    }
    // "cmd++" splits into an empty last part
    let key = key.or_else(|| combo.ends_with('+').then(|| "+".to_string())).ok_or("no key in shortcut")?;
    let using = if mods.is_empty() { String::new() } else { format!(" using {{{}}}", mods.join(", ")) };
    let code = match key.to_ascii_lowercase().as_str() {
        "return" | "enter" => Some(36),
        "tab" => Some(48),
        "space" => Some(49),
        "delete" | "backspace" => Some(51),
        "esc" | "escape" => Some(53),
        "left" => Some(123),
        "right" => Some(124),
        "down" => Some(125),
        "up" => Some(126),
        "home" => Some(115),
        "end" => Some(119),
        "pageup" => Some(116),
        "pagedown" => Some(121),
        "f1" => Some(122),
        "f2" => Some(120),
        "f3" => Some(99),
        "f4" => Some(118),
        "f5" => Some(96),
        "f6" => Some(97),
        "f7" => Some(98),
        "f8" => Some(100),
        "f9" => Some(101),
        "f10" => Some(109),
        "f11" => Some(103),
        "f12" => Some(111),
        _ => None,
    };
    Ok(match code {
        Some(c) => format!("tell application \"System Events\" to key code {c}{using}"),
        None if key.chars().count() == 1 => {
            let k = key.replace('\\', "\\\\").replace('"', "\\\"");
            format!("tell application \"System Events\" to keystroke \"{}\"{using}", k.to_lowercase())
        }
        None => return Err(format!("unknown key '{key}'")),
    })
}

// ------------------------------------------------------------------ mute

/// System output muted? `None` if it cannot be read on this OS.
pub fn get_muted() -> Option<bool> {
    // CoreAudio first: no process to start. AppleScript only for devices without a mute control.
    #[cfg(target_os = "macos")]
    if let Some(m) = mac::output_muted() {
        return Some(m);
    }
    #[cfg(target_os = "macos")]
    return output(Command::new("osascript").args(["-e", "output muted of (get volume settings)"])).and_then(|s| {
        match s.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    });
    #[cfg(all(unix, not(target_os = "macos")))]
    return output(Command::new("pactl").args(["get-sink-mute", "@DEFAULT_SINK@"])).map(|s| s.contains("yes"));
    #[allow(unreachable_code)]
    None
}

pub fn set_muted(m: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let r = output(Command::new("osascript").args(["-e", &format!("set volume output muted {m}")]));
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = output(Command::new("pactl").args(["set-sink-mute", "@DEFAULT_SINK@", if m { "1" } else { "0" }]));
    #[cfg(windows)]
    let r: Option<String> = {
        let _ = m;
        None
    };
    r.map(|_| ()).ok_or_else(|| "cannot change mute on this system".to_string())
}

/// Change the output volume: "+5", "-5", "40", "mute", "unmute", "toggle-mute".
pub fn change_volume(value: &str) -> Result<(), String> {
    let v = value.trim();
    match v {
        "mute" => return set_muted(true),
        "unmute" => return set_muted(false),
        "toggle-mute" => return set_muted(!get_muted().unwrap_or(false)),
        _ => {}
    }
    let num = |s: &str| s.parse::<i32>().map_err(|_| format!("volume: '{value}' is not a number"));
    #[cfg(target_os = "macos")]
    {
        let script = if let Some(d) = v.strip_prefix('+') {
            format!("set volume output volume ((output volume of (get volume settings)) + {})", num(d)?)
        } else if v.starts_with('-') {
            format!("set volume output volume ((output volume of (get volume settings)) - {})", -num(v)?)
        } else {
            format!("set volume output volume {}", num(v)?.clamp(0, 100))
        };
        output(Command::new("osascript").args(["-e", &script])).map(|_| ()).ok_or_else(|| "could not change the volume".into())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let arg = if v.starts_with('+') || v.starts_with('-') { format!("{}%", v) } else { format!("{}%", num(v)?.clamp(0, 150)) };
        output(Command::new("pactl").args(["set-sink-volume", "@DEFAULT_SINK@", &arg])).map(|_| ()).ok_or_else(|| "could not change the volume".into())
    }
    #[cfg(windows)]
    {
        let _ = num;
        Err("volume control is not supported on Windows yet".into())
    }
}

/// A short alert sound (pomodoro).
pub fn chime() {
    #[cfg(target_os = "macos")]
    let _ = run_shell("afplay /System/Library/Sounds/Glass.aiff", Path::new("/"));
    #[cfg(windows)]
    let _ = run_shell("powershell -c [console]::beep(880,300)", Path::new("."));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combos() {
        assert_eq!(
            applescript_for_combo("cmd+shift+4").unwrap(),
            "tell application \"System Events\" to keystroke \"4\" using {command down, shift down}"
        );
        assert_eq!(applescript_for_combo("ctrl+left").unwrap(), "tell application \"System Events\" to key code 123 using {control down}");
        assert_eq!(applescript_for_combo("F5").unwrap(), "tell application \"System Events\" to key code 96");
        assert!(applescript_for_combo("cmd+banana").is_err());
        assert!(applescript_for_combo("cmd").is_err());
    }

    #[test]
    fn app_matching() {
        let a = AppInfo { name: "Safari".into(), id: "com.apple.Safari".into() };
        assert!(a.matches("com.apple.safari"));
        assert!(a.matches("Safari"));
        assert!(!a.matches("Chrome"));
        let w = AppInfo { name: "chrome".into(), id: "C:\\Program Files\\Google\\chrome.exe".into() };
        assert!(w.matches("chrome.exe"));
        assert!(w.matches("Chrome"));
    }
}
