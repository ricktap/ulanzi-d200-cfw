//! Deck configuration.
//!
//! A config holds profiles; each profile has pages of keys 0-12 (key 13, the wide
//! window, is the status key and is configured once in `status_key`). A key is
//! either a button with one or more *states* (each with its own look and its own
//! actions per event) or a built-in *app* (system monitor, mute, weather, clock,
//! pomodoro, ...).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const NUM_KEYS: usize = 14;
pub const STATUS_KEY: usize = 13;

fn is_false(b: &bool) -> bool {
    !*b
}

/// How a key looks.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Look {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// An emoji, or a path to an image, drawn above the label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Path to an image that fills the whole key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// State of a stateful icon (light, thermostat, pomodoro, analog-clock,
    /// bambu-ams, bambu-ams-lite, downloads, volume-muted), e.g.
    /// {"on": true, "color": "#B79AF7"} or {"temperature": 21.5, "progress": 60}.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_state: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,
}

impl Look {
    pub fn is_empty(&self) -> bool {
        *self == Look::default()
    }
}

/// One thing a key can do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Shell command on this computer.
    Command { command: String },
    /// App name, file, folder or URL.
    Open { target: String },
    /// Keyboard shortcut such as "cmd+shift+4".
    Keys { combo: String },
    /// Go to a page of the current profile.
    Page { page: String },
    /// Previous page.
    Back,
    /// First page of the current profile.
    Home,
    /// Switch profile: a profile name, "next", or "auto" (follow the active app again).
    Profile { profile: String },
    /// Change this key's state: "next" or a state number (0-based).
    State { to: String },
    /// Publish to MQTT.
    Mqtt {
        topic: String,
        payload: String,
        #[serde(default, skip_serializing_if = "is_false")]
        retain: bool,
    },
    /// "128" sets, "+25" / "-25" adjusts.
    Brightness { value: String },
    /// Show a notification on the status key.
    Notify {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        icon: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<String>,
    },
    /// Show the Home Assistant remote page.
    Remote,
    /// Spotify / Music: "playpause", "next", "previous", "play", "pause".
    Media { command: String },
    /// Teams meeting: "toggle-mute", "toggle-video", "toggle-hand",
    /// "toggle-background-blur", "leave-call", "toggle-ui-chat".
    Teams { command: String },
    /// System output volume: "+5" / "-5" (percent steps), "40" (set), "mute", "unmute", "toggle-mute".
    Volume { value: String },
    /// Teams presence: "toggle", "auto", or "Available", "Busy", "DoNotDisturb",
    /// "BeRightBack", "Away", "Offline".
    Presence { to: String },
}

/// Actions per key event. All four can be used together:
/// `down` and `up` fire on every press; exactly one of `press` (released
/// before the long-press time) or `long` (held past it) fires per press.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Actions {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub press: Vec<Action>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub long: Vec<Action>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub down: Vec<Action>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub up: Vec<Action>,
}

impl Actions {
    pub fn is_empty(&self) -> bool {
        self.press.is_empty() && self.long.is_empty() && self.down.is_empty() && self.up.is_empty()
    }
    pub fn get(&self, ev: KeyEvent) -> &[Action] {
        match ev {
            KeyEvent::Press => &self.press,
            KeyEvent::Long => &self.long,
            KeyEvent::Down => &self.down,
            KeyEvent::Up => &self.up,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyEvent {
    Down,
    Up,
    Press,
    Long,
}

impl KeyEvent {
    pub fn name(self) -> &'static str {
        match self {
            KeyEvent::Down => "down",
            KeyEvent::Up => "up",
            KeyEvent::Press => "press",
            KeyEvent::Long => "long_press",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    #[serde(flatten)]
    pub look: Look,
    #[serde(skip_serializing_if = "Actions::is_empty")]
    pub actions: Actions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppKind {
    Sysmon,
    Mute,
    Weather,
    Clock,
    Pomodoro,
    /// Play/pause with status (Spotify / Music).
    Media,
    TeamsMute,
    TeamsCamera,
    TeamsPresence,
    /// JetBrains IDE debugger.
    Debugger,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppCfg {
    #[serde(rename = "type")]
    pub kind: AppKind,
    /// System monitor: which views a press cycles through
    /// ("cpu", "ram", "disk", "net"). Empty = all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modes: Vec<String>,
    /// Clock: 12-hour format.
    #[serde(default, skip_serializing_if = "is_false")]
    pub h12: bool,
    /// Debugger: shortcut on press while idle (default "ctrl+d", Debug).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_keys: Option<String>,
    /// Debugger: shortcut on press while debugging (default "cmd+alt+r", Resume).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_keys: Option<String>,
    /// Debugger: shortcut on long press while debugging (default "cmd+f2", Stop).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_keys: Option<String>,
}

impl AppCfg {
    pub fn new(kind: AppKind) -> Self {
        AppCfg { kind, modes: vec![], h12: false, idle_keys: None, active_keys: None, stop_keys: None }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Key {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<AppCfg>,
    /// Button states. State 0 is the initial one; apps use state 0 for colours
    /// and extra actions.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<State>,
}

impl Key {
    pub fn is_empty(&self) -> bool {
        self.app.is_none() && self.states.iter().all(|s| s.look.is_empty() && s.actions.is_empty())
    }
    pub fn state(&self, i: usize) -> State {
        if self.states.is_empty() {
            State::default()
        } else {
            self.states[i % self.states.len()].clone()
        }
    }
}

pub type Page = IndexMap<String, Key>;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    /// Apps this profile is used for (bundle id such as "com.apple.Safari",
    /// or app / exe name). Empty for the default profile.
    pub apps: Vec<String>,
    /// Ordered: the first page is the profile's home page.
    pub pages: IndexMap<String, Page>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MqttCfg {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub tls: bool,
    pub base_topic: String,
    /// Publish Home Assistant MQTT discovery (device triggers, notify, profile select...).
    pub discovery: bool,
}

impl Default for MqttCfg {
    fn default() -> Self {
        MqttCfg {
            enabled: false,
            host: "homeassistant.local".into(),
            port: 1883,
            username: String::new(),
            password: String::new(),
            tls: false,
            base_topic: "d200".into(),
            discovery: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WeatherCfg {
    pub location: Option<Location>,
    pub fahrenheit: bool,
}

impl Default for WeatherCfg {
    fn default() -> Self {
        WeatherCfg { location: None, fahrenheit: false }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PomodoroCfg {
    pub work: u32,
    pub short_break: u32,
    pub long_break: u32,
    /// Work rounds before a long break.
    pub rounds: u32,
    pub sound: bool,
}

impl Default for PomodoroCfg {
    fn default() -> Self {
        PomodoroCfg { work: 25, short_break: 5, long_break: 15, rounds: 4, sound: true }
    }
}

/// Semantic palette (THEMING.md: --accent-active, -info, -attention, -error, --widget-background).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Palette {
    pub active: String,
    pub info: String,
    pub attention: String,
    pub error: String,
    pub background: String,
}

impl Default for Palette {
    fn default() -> Self {
        Palette {
            active: "#69E0C1".into(),
            info: "#B79AF7".into(),
            attention: "#FFBF69".into(),
            error: "#FF8278".into(),
            background: "#11171D".into(),
        }
    }
}

/// Per-widget overrides (THEMING.md per-widget variables). Unset = from the palette.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WidgetTheme {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light: Option<String>,
    /// 0..1, default 0.9; 0 disables the background light.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub light_opacity: Option<f32>,
    /// Light multiplier while idle, default 0.28.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idle_light_factor: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_accent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_up: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_down: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_tint: Option<String>,
    /// Wide cards: "compact" (default) or "airy".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    pub palette: Palette,
    /// Keyed by widget: cpu, ram, disk, network, clock, mail, music, printer, meeting, game, push.
    pub widgets: IndexMap<String, WidgetTheme>,
}

/// Which events may show a card on the wide display.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CardsCfg {
    pub email: bool,
    pub meetings: bool,
    pub steam: bool,
    pub printer: bool,
    /// Privacy: "New email" instead of sender and subject.
    pub hide_content: bool,
}

impl Default for CardsCfg {
    fn default() -> Self {
        CardsCfg { email: true, meetings: true, steam: true, printer: true, hide_content: false }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScreensaverCfg {
    pub enabled: bool,
    /// Minutes without a key press before the scene starts.
    pub idle_minutes: u32,
    /// "auto" (by season), "snow", "pink", "beach", "leaves", "rain".
    pub scene: String,
    /// Southern hemisphere seasons for "auto".
    pub southern: bool,
    /// false = a still scene (reduced motion).
    pub motion: bool,
    /// Frames per second sent to the device (each frame is up to 1.6 MB over USB).
    pub fps: u32,
}

impl Default for ScreensaverCfg {
    fn default() -> Self {
        ScreensaverCfg { enabled: true, idle_minutes: 3, scene: "auto".into(), southern: false, motion: true, fps: 8 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TeamsCfg {
    /// Port of the Teams third-party app API.
    pub port: u16,
    /// Entra (Azure AD) app registration for presence: public client with
    /// device-code flow and delegated Presence.ReadWrite.
    pub client_id: String,
    /// "organizations", or your tenant id / domain.
    pub tenant: String,
    /// What "unavailable" means for the presence toggle.
    pub away: String,
}

impl Default for TeamsCfg {
    fn default() -> Self {
        TeamsCfg { port: 8124, client_id: String::new(), tenant: "organizations".into(), away: "DoNotDisturb".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub brightness: u8,
    pub long_press_ms: u64,
    /// Switch profiles to match the frontmost app.
    pub follow_active_app: bool,
    pub weather: WeatherCfg,
    pub pomodoro: PomodoroCfg,
    pub mqtt: MqttCfg,
    pub teams: TeamsCfg,
    pub screensaver: ScreensaverCfg,
    pub cards: CardsCfg,
    pub theme: Theme,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            brightness: 180,
            long_press_ms: 500,
            follow_active_app: true,
            weather: WeatherCfg::default(),
            pomodoro: PomodoroCfg::default(),
            mqtt: MqttCfg::default(),
            teams: TeamsCfg::default(),
            screensaver: ScreensaverCfg::default(),
            cards: CardsCfg::default(),
            theme: Theme::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusKey {
    pub actions: Actions,
    /// Default content: "profile", "media", "meeting", "email", "steam", "printer".
    pub app: String,
    /// "auto" (the song while Spotify/Music plays, else the profile),
    /// "profile" or "media".
    pub show: String,
}

impl Default for StatusKey {
    fn default() -> Self {
        StatusKey {
            actions: Actions { press: vec![Action::Home], long: vec![Action::Remote], ..Default::default() },
            app: "profile".into(),
            show: "auto".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub status_key: StatusKey,
    /// The first profile is the default: used when no other profile matches the active app.
    pub profiles: Vec<Profile>,
}

impl Config {
    pub fn example() -> Self {
        serde_json::from_str(include_str!("default-config.json")).expect("bundled example config")
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        let cfg: Config = serde_json::from_str(s).map_err(|e| e.to_string())?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.profiles.is_empty() {
            return Err("a config needs at least one profile".into());
        }
        for p in &self.profiles {
            if p.name.trim().is_empty() {
                return Err("profile names cannot be empty".into());
            }
            if p.pages.is_empty() {
                return Err(format!("profile '{}' has no pages", p.name));
            }
            for (name, page) in &p.pages {
                if name.trim().is_empty() {
                    return Err(format!("profile '{}': page names cannot be empty", p.name));
                }
                for k in page.keys() {
                    match k.parse::<usize>() {
                        Ok(i) if i < STATUS_KEY => {}
                        _ => return Err(format!("profile '{}', page '{name}': key '{k}' is not 0-12", p.name)),
                    }
                }
            }
        }
        let mut names: Vec<&str> = self.profiles.iter().map(|p| p.name.as_str()).collect();
        names.sort();
        names.dedup();
        if names.len() != self.profiles.len() {
            return Err("profile names must be unique".into());
        }
        Ok(())
    }

    /// Does any key use one of these apps, or any action match `act`?
    pub fn uses(&self, kinds: &[AppKind], act: impl Fn(&Action) -> bool) -> bool {
        self.profiles.iter().flat_map(|p| p.pages.values()).flat_map(|pg| pg.values()).any(|k| {
            k.app.as_ref().is_some_and(|a| kinds.contains(&a.kind))
                || k.states.iter().any(|s| {
                    [&s.actions.press, &s.actions.long, &s.actions.down, &s.actions.up].iter().any(|l| l.iter().any(&act))
                })
        })
    }

    pub fn profile_index(&self, name: &str) -> Option<usize> {
        self.profiles.iter().position(|p| p.name == name)
    }

    pub fn key(&self, profile: usize, page: &str, key: usize) -> Key {
        self.profiles
            .get(profile)
            .and_then(|p| p.pages.get(page))
            .and_then(|pg| pg.get(&key.to_string()))
            .cloned()
            .unwrap_or_default()
    }

    /// Load `path`, creating it from the example config if it does not exist.
    pub fn load_or_create(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(s) => match Config::parse(&s) {
                Ok(c) => Ok(c),
                Err(e) => {
                    // Draft phase: no migrations. Keep the old file aside and start fresh.
                    let _ = std::fs::rename(path, path.with_extension("json.old"));
                    let c = Config::example();
                    c.save(path)?;
                    eprintln!("{}: {e}; moved to deck.json.old, started from the example", path.display());
                    Ok(c)
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let c = Config::example();
                c.save(path)?;
                Ok(c)
            }
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        write_private(path, &(json + "\n"))
    }
}

/// Write atomically; owner-only permissions because the file can hold an MQTT password.
pub fn write_private(path: &Path, data: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, data).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Resolve an image path from the config: absolute, `~/...`, or relative to the config folder.
pub fn resolve(base: &Path, p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            return PathBuf::from(home).join(rest);
        }
    }
    let pb = PathBuf::from(p);
    if pb.is_absolute() {
        pb
    } else {
        base.join(pb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_parses_and_validates() {
        let c = Config::example();
        c.validate().unwrap();
        // round trip is stable
        let s = serde_json::to_string(&c).unwrap();
        assert_eq!(Config::parse(&s).unwrap(), c);
    }

    /// A config as the UI writes it (empty action lists, every action type).
    #[test]
    fn parses_ui_output() {
        let c = Config::parse(include_str!("testdata/ui-config.json")).unwrap();
        let k5 = &c.profiles[0].pages["main"]["5"].states[0].actions;
        assert_eq!(k5.down, vec![Action::Mqtt { topic: "home/x".into(), payload: "ON".into(), retain: true }]);
        assert_eq!(c.profiles[1].apps, vec!["com.microsoft.VSCode".to_string()]);
        // empty lists are dropped when written back
        let out = serde_json::to_string(&c).unwrap();
        assert!(!out.contains("\"press\":[]"));
    }

    #[test]
    fn rejects_key_13_and_duplicates() {
        let mut c = Config::example();
        c.profiles[0].pages[0].insert("13".into(), Key::default());
        assert!(c.validate().is_err());
        let mut c = Config::example();
        let p = c.profiles[0].clone();
        c.profiles.push(p);
        assert!(c.validate().is_err());
    }
}

#[cfg(test)]
mod reset_tests {
    use super::*;

    /// An old or broken deck.json is moved aside and replaced by the example.
    #[test]
    fn unparsable_config_starts_fresh() {
        let dir = std::env::temp_dir().join(format!("d200deck-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("deck.json");
        std::fs::write(&path, r#"{"pages": {"main": {}}}"#).unwrap();
        let c = Config::load_or_create(&path).unwrap();
        assert_eq!(c, Config::example());
        assert!(dir.join("deck.json.old").exists());
        assert_eq!(Config::load_or_create(&path).unwrap(), Config::example());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
