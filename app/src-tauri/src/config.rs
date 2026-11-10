//! Deck configuration.
//!
//! A config holds profiles; each profile has pages of keys 0-12 (key 13, the wide
//! window, is the status key and is configured once in `status_key`). A key is
//! either a button with one or more *states* (each with its own look and its own
//! actions per event) or a built-in *app* (system monitor, mute, weather, clock,
//! pomodoro, ...).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
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
    /// Static interface properties for the icon, e.g. {"toggle.on": true,
    /// "rgb.color": "#B79AF7"}. Used by keys without an app, remote keys and cards;
    /// an app supplies these itself.
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub props: Map<String, Value>,
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

impl Action {
    /// The id of a core action: `core:<type>`.
    pub fn to_ref(&self) -> ActionRef {
        let mut v = serde_json::to_value(self).unwrap_or_default();
        let obj = v.as_object_mut().map(std::mem::take).unwrap_or_default();
        let mut params = obj;
        let kind = params.remove("type").and_then(|t| t.as_str().map(str::to_string)).unwrap_or_default();
        ActionRef { action: format!("core:{kind}"), params }
    }
}

/// An action as stored: `{"action": "core:page", "params": {"page": "Media"}}`.
/// Actions of unknown plugins are kept as they are.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ActionRef {
    pub action: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub params: Map<String, Value>,
}

impl ActionRef {
    /// The core action this names, or `None` for another plugin's action or bad params.
    pub fn parse(&self) -> Option<Action> {
        let kind = self.action.strip_prefix("core:")?;
        let mut obj = self.params.clone();
        obj.insert("type".into(), Value::String(kind.into()));
        serde_json::from_value(Value::Object(obj)).ok()
    }
    pub fn plugin(&self) -> &str {
        self.action.split_once(':').map(|(p, _)| p).unwrap_or("core")
    }
}

impl From<Action> for ActionRef {
    fn from(a: Action) -> Self {
        a.to_ref()
    }
}

/// Actions per key event. All four can be used together:
/// `down` and `up` fire on every press; exactly one of `press` (released
/// before the long-press time) or `long` (held past it) fires per press.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Actions {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub press: Vec<ActionRef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub long: Vec<ActionRef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub down: Vec<ActionRef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub up: Vec<ActionRef>,
}

impl Actions {
    pub fn is_empty(&self) -> bool {
        self.press.is_empty() && self.long.is_empty() && self.down.is_empty() && self.up.is_empty()
    }
    pub fn get(&self, ev: KeyEvent) -> &[ActionRef] {
        match ev {
            KeyEvent::Press => &self.press,
            KeyEvent::Long => &self.long,
            KeyEvent::Down => &self.down,
            KeyEvent::Up => &self.up,
        }
    }
    pub fn all(&self) -> impl Iterator<Item = &ActionRef> {
        self.press.iter().chain(&self.long).chain(&self.down).chain(&self.up)
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
    pub on: Actions,
    /// Fields this version does not know, kept on save.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
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

impl AppKind {
    pub const ALL: [AppKind; 10] = [
        AppKind::Sysmon,
        AppKind::Mute,
        AppKind::Weather,
        AppKind::Clock,
        AppKind::Pomodoro,
        AppKind::Media,
        AppKind::TeamsMute,
        AppKind::TeamsCamera,
        AppKind::TeamsPresence,
        AppKind::Debugger,
    ];
    /// "core:sysmon" etc.
    pub fn id(self) -> String {
        format!("core:{}", serde_json::to_value(self).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default())
    }
    pub fn from_id(id: &str) -> Option<AppKind> {
        let name = id.strip_prefix("core:")?;
        serde_json::from_value(Value::String(name.into())).ok()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Key {
    /// App id such as "core:sysmon". None: a plain button (`core:button`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// The app's settings for this key (schema in the app's `config`).
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub config: Map<String, Value>,
    /// Button states. State 0 is the initial one; apps use state 0 for the chosen
    /// icon, label, colours and actions.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<State>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Key {
    pub fn is_empty(&self) -> bool {
        self.app.is_none() && self.states.iter().all(|s| s.look.is_empty() && s.on.is_empty())
    }
    /// The built-in app this key runs, if it is one of the core apps.
    pub fn kind(&self) -> Option<AppKind> {
        self.app.as_deref().and_then(AppKind::from_id)
    }
    pub fn cfg_str(&self, k: &str) -> Option<&str> {
        self.config.get(k).and_then(Value::as_str).filter(|s| !s.trim().is_empty())
    }
    pub fn cfg_bool(&self, k: &str) -> bool {
        self.config.get(k).and_then(Value::as_bool).unwrap_or(false)
    }
    pub fn cfg_list(&self, k: &str) -> Vec<String> {
        self.config.get(k).and_then(Value::as_array).into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect()
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
    #[serde(flatten)]
    pub extra: Map<String, Value>,
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
    /// Theme variables over the palette: `accent`, `background`, ... or any
    /// namespaced name. A value is a colour or a binding (`{"bind": ..., "steps": [...]}`).
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub variables: Map<String, Value>,
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
    #[serde(flatten)]
    pub extra: Map<String, Value>,
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
            extra: Map::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusKey {
    pub on: Actions,
    /// Default content: "profile", "media", "meeting", "email", "steam", "printer".
    pub app: String,
    /// "auto" (the song while Spotify/Music plays, else the profile),
    /// "profile" or "media".
    pub show: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Default for StatusKey {
    fn default() -> Self {
        StatusKey {
            on: Actions { press: vec![Action::Home.into()], long: vec![Action::Remote.into()], ..Default::default() },
            app: "profile".into(),
            show: "auto".into(),
            extra: Map::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// Format version; see `migrate`.
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub status_key: StatusKey,
    /// The first profile is the default: used when no other profile matches the active app.
    pub profiles: Vec<Profile>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

/// The current deck.json format.
pub const VERSION: u32 = 2;

impl Config {
    pub fn example() -> Self {
        Config::parse(include_str!("default-config.json")).expect("bundled example config")
    }

    /// Parse any known version; older files are migrated in memory.
    pub fn parse(s: &str) -> Result<Self, String> {
        Ok(Config::parse_versioned(s)?.0)
    }

    /// Like `parse`, also returning the version the text was in.
    pub fn parse_versioned(s: &str) -> Result<(Self, u64), String> {
        Config::from_value(serde_json::from_str(s).map_err(|e| e.to_string())?)
    }

    /// From JSON of any known version (the UI, imports).
    pub fn from_value(v: Value) -> Result<(Self, u64), String> {
        let was = crate::migrate::version(&v);
        let v = crate::migrate::migrate(v, VERSION)?;
        let cfg: Config = serde_json::from_value(v).map_err(|e| e.to_string())?;
        cfg.validate()?;
        Ok((cfg, was))
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
            k.kind().is_some_and(|kd| kinds.contains(&kd))
                || k.states.iter().any(|s| s.on.all().filter_map(ActionRef::parse).any(|a| act(&a)))
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
    /// An older version is migrated; the original stays next to it as `deck.v<N>.json`.
    /// A file that cannot be read is moved to `deck.json.old` and the example is used.
    pub fn load_or_create(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(s) => match Config::parse_versioned(&s) {
                Ok((c, was)) => {
                    if was < VERSION as u64 {
                        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("deck");
                        let backup = path.with_file_name(format!("{stem}.v{was}.json"));
                        write_private(&backup, &s)?;
                        c.save(path)?;
                    }
                    Ok(c)
                }
                Err(e) => {
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
        let mut c = self.clone();
        c.version = VERSION;
        let json = serde_json::to_string_pretty(&c).map_err(|e| e.to_string())?;
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
        let k5 = &c.profiles[0].pages["main"]["5"].states[0].on;
        assert_eq!(k5.down[0].parse(), Some(Action::Mqtt { topic: "home/x".into(), payload: "ON".into(), retain: true }));
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
mod version_tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("d200deck-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A version 1 file is migrated, saved as version 2, and the original kept.
    #[test]
    fn v1_file_is_migrated_with_backup() {
        let dir = tmp("v1");
        let path = dir.join("deck.json");
        let v1 = r#"{"profiles": [{"name": "Default", "pages": {"main": {
            "0": {"app": {"type": "clock", "h12": true}},
            "1": {"states": [{"icon": "volume-on", "actions": {"press": [{"type": "volume", "value": "+5"}]}}]}}}}]}"#;
        std::fs::write(&path, v1).unwrap();
        let c = Config::load_or_create(&path).unwrap();
        let k0 = c.key(0, "main", 0);
        assert_eq!(k0.kind(), Some(AppKind::Clock));
        assert!(k0.cfg_bool("h12"));
        let k1 = c.key(0, "main", 1);
        assert_eq!(k1.states[0].look.icon.as_deref(), Some("core:volume-on"));
        assert_eq!(k1.states[0].on.press[0].parse(), Some(Action::Volume { value: "+5".into() }));
        assert_eq!(std::fs::read_to_string(dir.join("deck.v1.json")).unwrap(), v1);
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["version"], 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Unknown apps, actions and fields survive loading and saving unchanged.
    #[test]
    fn unknown_things_are_kept() {
        let key = serde_json::json!({
            "app": "com.ricktap.bambu:ams",
            "config": {"unit": "ams2"},
            "states": [{"icon": "com.jemand.neon:spools", "label": "AMS 2", "glow": 3,
                        "on": {"long": [{"action": "com.ricktap.bambu:refresh", "params": {"all": true}}]}}],
            "future": {"x": 1}
        });
        let text = serde_json::json!({
            "version": 2, "overlays": ["com.jemand.ams-blau"],
            "settings": {"new_setting": 5},
            "profiles": [{"name": "Default", "color": "red", "pages": {"main": {"6": key}}}]
        })
        .to_string();
        let c = Config::parse(&text).unwrap();
        let out: Value = serde_json::to_value(&c).unwrap();
        assert_eq!(out["profiles"][0]["pages"]["main"]["6"], key);
        assert_eq!(out["overlays"], serde_json::json!(["com.jemand.ams-blau"]));
        assert_eq!(out["settings"]["new_setting"], 5);
        assert_eq!(out["profiles"][0]["color"], "red");
        assert_eq!(c.key(0, "main", 6).kind(), None);
        assert_eq!(c.key(0, "main", 6).states[0].on.long[0].parse(), None);
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
