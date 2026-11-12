//! The deck: one event-loop thread owns all state (config, device, apps, MQTT,
//! notifications) and reacts to messages. Independent of Tauri; the app passes
//! an `emit` callback for UI events and talks to it through `Handle`.

use crate::apps;
use crate::bind::{self, Ctx, Reads, Signals};
use crate::render::IconEnv;
use crate::plugins::{Host, Plugin};
use serde_json::Map;
use crate::cards::{self, Card, Kind, Printer};
use std::collections::HashSet;
use crate::plugins::jetbrains::ide::DebugWatch;
use crate::screensaver::{Scene, Season};
use crate::plugins::core::media::{self, NowPlaying};
use crate::plugins::teams::api::{self as teams, Tokens};
use image::DynamicImage;
use crate::config::{Action, AppKind, Config, Key, KeyEvent, NUM_KEYS, STATUS_KEY};
use crate::device::{self, DevEvent, Device};
use crate::mqtt::{self, Mqtt, MqttMsg};
use crate::platform::{self, AppInfo};
use crate::render::{self, Canvas, Fonts, Notification, StatusInfo};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub type Emit = Box<dyn Fn(&str, Value) + Send + Sync>;
type Reply<T> = Sender<T>;

const FRAME: Duration = Duration::from_millis(80);
const OWN_APP_IDS: [&str; 3] = ["dev.d200fw.deck", "D200 Deck", "d200-deck"];

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Status {
    pub connected: bool,
    pub version: Option<String>,
    pub adb: Option<String>,
    pub device_seen: bool,
    pub message: String,
    /// None = MQTT off, Some(true) connected, Some(false) trying.
    pub mqtt_ok: Option<bool>,
    pub mqtt: String,
    pub active_app: Option<AppInfo>,
}

pub enum Ui {
    Snapshot(Reply<Value>),
    Save(Box<Config>, Reply<Result<Value, String>>),
    View { profile: usize, page: String, remote: bool },
    Preview { profile: usize, page: String, remote: bool, key: usize, reply: Reply<Result<String, String>> },
    Trigger { key: usize, event: KeyEvent },
    Notify(Notification),
    /// Read the packages folder again; replies with the warnings of the new registry.
    ReloadPackages(Reply<Vec<String>>),
    /// Graph sign-in finished (Some) or sign-out (None).
    GraphTokens(Option<Tokens>),
    Screensaver(bool),
    Options { source: String, profile: usize, reply: Reply<Vec<Value>> },
    Signals(Reply<Value>),
    PreviewIcon { icon: String, profile: usize, page: String, key: usize, reply: Reply<Result<String, String>> },
}

pub enum Msg {
    Dev(DevEvent),
    DevUp(Arc<Device>),
    Conn(Status),
    Mqtt(u64, MqttMsg),
    Ui(Ui),
    Error(String),
    Art(String, Option<DynamicImage>),
    /// A built-in plugin's background work reporting back (see plugins.rs).
    Plugin(&'static str, crate::plugins::PluginMsg),
    SteamCover(u32, Option<DynamicImage>),
}

/// Thread-safe handle used by the Tauri commands.
#[derive(Clone)]
pub struct Handle {
    tx: Sender<Msg>,
    cfg_dir: PathBuf,
    firmware: &'static [u8],
}

impl Handle {
    pub fn send(&self, ui: Ui) {
        let _ = self.tx.send(Msg::Ui(ui));
    }

    pub fn ask<T>(&self, f: impl FnOnce(Reply<T>) -> Ui) -> Result<T, String> {
        let (rtx, rrx) = mpsc::channel();
        self.send(f(rtx));
        rrx.recv_timeout(Duration::from_secs(20)).map_err(|_| "the deck engine did not answer".to_string())
    }

    pub fn config_dir(&self) -> PathBuf {
        self.cfg_dir.clone()
    }

    /// Copy the bundled firmware to the D200 and start it (until reboot). Blocking.
    pub fn start_firmware(&self) -> Result<String, String> {
        let adb = device::adb_path().ok_or("adb not found")?;
        if !device::adb_device_present(&adb) {
            return Err("No D200 connected over USB.".into());
        }
        device::start_firmware(&adb, self.firmware)
    }
}

// ------------------------------------------------------------------ runtime state (state.json)

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Runtime {
    /// "profile/page/key" -> current state index
    states: HashMap<String, usize>,
    /// "profile/page/key" -> app mode (system monitor view, clock time/date)
    modes: HashMap<String, usize>,
}

#[derive(Clone, Copy)]
struct Held {
    since: Instant,
    long_fired: bool,
}

#[derive(Clone, PartialEq)]
struct View {
    profile: usize,
    page: String,
    remote: bool,
}

pub struct Engine {
    cfg: Config,
    path: PathBuf,
    rt: Runtime,
    rt_path: PathBuf,
    rt_dirty: bool,
    view: View,
    stack: Vec<String>,
    manual: bool,

    dev: Option<Arc<Device>>,
    connected: Arc<AtomicBool>,
    sent: [u64; NUM_KEYS],
    /// The last image sent per key, to send only the part that changed.
    last: Vec<Vec<u8>>,
    status: Status,
    emit: Emit,
    fonts: Result<Fonts, String>,
    tx: Sender<Msg>,

    held: [Option<Held>; NUM_KEYS],
    next_tick: Instant,
    ticks: u64,


    notes: VecDeque<Card>,
    note: Option<(Card, Instant)>,
    next_frame: Instant,

    /// Built-in plugins, called in this thread (taken out while one runs).
    plugins: Vec<Option<Box<dyn Plugin>>>,
    mqtt: Option<Mqtt>,
    mqtt_gen: u64,
    mqtt_up: bool,
    avail_owner: String,
    host: String,

    np: Option<NowPlaying>,
    np_at: Instant,
    art: Option<(String, Option<Arc<DynamicImage>>)>,
    art_busy: bool,
    profile_flash: Option<Instant>,


    dbg: Option<DebugWatch>,
    /// The front app at the last Steam scan.
    steam_front: Option<String>,

    mail_seen: Option<Option<String>>,
    /// Teams pairing as last seen, for the "Paired with Teams" card.
    paired_seen: Option<bool>,
    /// Pomodoro phase ends seen, for its cards.
    pomo_seen: Option<u64>,
    mail_dismissed: Option<String>,
    announced: HashSet<String>,
    meeting_dismissed: Option<String>,
    game: Option<crate::plugins::jetbrains::ide::Game>,
    game_cover: Option<(u32, Option<Arc<DynamicImage>>)>,
    game_hidden: bool,
    printer: Option<Printer>,
    status_capture: Option<Visible>,

    last_input: Instant,
    saver: Option<Saver>,
    scene: Scene,
    /// Keys whose press woke the screensaver: their up/press/long are swallowed.
    swallow: [bool; NUM_KEYS],
    /// A press waiting to see whether a second one makes it a double press (until then).
    pending_press: [Option<Instant>; NUM_KEYS],
    /// The d200fw this app embeds, to upgrade an older one on the device (once per run).
    firmware: &'static [u8],
    fw_upgrade_tried: bool,

    /// Latest value per signal (the core's own for now).
    signals: Signals,
    /// What each key's icon read when it was last drawn.
    reads: Vec<Reads>,
}

/// What the wide key showed when it was pressed (presses act on that).
#[derive(Clone)]
enum Visible {
    Note(Card),
    App(String),
    Profile,
}

struct Saver {
    since: Instant,
    season: Season,
    next_frame: Instant,
    t: f32,
    pm: tiny_skia::Pixmap,
    /// The device draws the scene itself (d200fw 0.2+); the host only sends the dot.
    on_device: bool,
    dot: bool,
}

/// Bounding box (x, y, w, h) of the pixels that differ between two RGB images of width
/// `w`; None when they are equal.
fn changed_box(a: &[u8], b: &[u8], w: usize) -> Option<(usize, usize, usize, usize)> {
    let h = a.len() / 3 / w;
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    for y in 0..h {
        let (ra, rb) = (&a[y * w * 3..(y + 1) * w * 3], &b[y * w * 3..(y + 1) * w * 3]);
        if ra == rb {
            continue;
        }
        let first = (0..w).find(|&x| ra[x * 3..x * 3 + 3] != rb[x * 3..x * 3 + 3]).unwrap_or(0);
        let last = (0..w).rev().find(|&x| ra[x * 3..x * 3 + 3] != rb[x * 3..x * 3 + 3]).unwrap_or(w - 1);
        x0 = x0.min(first);
        x1 = x1.max(last);
        y0 = y0.min(y);
        y1 = y;
    }
    (y0 != usize::MAX).then(|| (x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}

fn hash(v: &[u8]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

fn base_dir(p: &Path) -> PathBuf {
    p.parent().map(Path::to_path_buf).unwrap_or_default()
}

impl Engine {
    /// Start the engine thread and the device connector. Returns the handle.
    pub fn spawn(path: PathBuf, emit: Emit, firmware: &'static [u8]) -> Handle {
        let (tx, rx) = mpsc::channel();
        let cfg_dir = base_dir(&path);
        // icon sets and themes from <config dir>/packages
        crate::registry::load_disk(&cfg_dir.join("packages"));
        let handle = Handle { tx: tx.clone(), cfg_dir, firmware };
        let connected = Arc::new(AtomicBool::new(false));

        // Device events -> engine messages.
        let (dtx, drx) = mpsc::channel::<DevEvent>();
        {
            let tx = tx.clone();
            std::thread::spawn(move || {
                for ev in drx {
                    if tx.send(Msg::Dev(ev)).is_err() {
                        return;
                    }
                }
            });
        }
        // Connector: keeps trying while no device is connected.
        {
            let tx = tx.clone();
            let connected = connected.clone();
            // Without a device it waits for adb to report one (`adb track-devices`)
            // instead of starting adb every few seconds.
            let (wake_tx, wake) = mpsc::channel::<()>();
            let direct = std::env::var_os("D200_DIRECT").is_some();
            if let (false, Some(adb)) = (direct, device::adb_path()) {
                device::track_devices(adb, move || wake_tx.send(()).is_ok());
            }
            std::thread::spawn(move || {
                let next_id = AtomicU64::new(1);
                loop {
                    // connected: only watch the flag (cheap), to notice a drop quickly
                    let up = connected.load(Ordering::SeqCst);
                    let mut wait = Duration::from_secs(if up { 2 } else { 30 });
                    if !up {
                        let msg = match try_connect(next_id.fetch_add(1, Ordering::SeqCst), dtx.clone()) {
                            Ok(dev) => Msg::DevUp(Arc::new(dev)),
                            Err(st) => {
                                // the D200 is there but d200fw does not answer yet: retry soon
                                if st.device_seen || direct {
                                    wait = Duration::from_secs(if direct { 2 } else { 5 });
                                }
                                Msg::Conn(st)
                            }
                        };
                        if tx.send(msg).is_err() {
                            return;
                        }
                    }
                    // a device change, or the time (no adb watcher: just the time)
                    if let Err(mpsc::RecvTimeoutError::Disconnected) = wake.recv_timeout(wait) {
                        std::thread::sleep(wait);
                    }
                    while wake.try_recv().is_ok() {}
                }
            });
        }

        std::thread::spawn(move || {
            let mut e = Engine::new(path, emit, tx, connected);
            e.firmware = firmware;
            e.run(rx);
        });
        handle
    }

    fn new(path: PathBuf, emit: Emit, tx: Sender<Msg>, connected: Arc<AtomicBool>) -> Engine {
        let (cfg, message) = match Config::load_or_create(&path) {
            Ok(c) => (c, "Starting".to_string()),
            Err(e) => (Config::example(), format!("Config error, using the example config: {e}")),
        };
        let rt_path = base_dir(&path).join("state.json");
        let rt = std::fs::read_to_string(&rt_path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let page = cfg.profiles[0].pages.keys().next().cloned().unwrap_or_default();
        let now = Instant::now();
        let mut e = Engine {
            cfg,
            path,
            rt,
            rt_path,
            rt_dirty: false,
            view: View { profile: 0, page, remote: false },
            stack: vec![],
            manual: false,
            dev: None,
            connected,
            sent: [0; NUM_KEYS],
            last: vec![Vec::new(); NUM_KEYS],
            status: Status { message, ..Default::default() },
            emit,
            fonts: Fonts::load(),
            tx,
            held: [None; NUM_KEYS],
            next_tick: now,
            ticks: 0,
            notes: VecDeque::new(),
            note: None,
            next_frame: now,
            plugins: vec![
                Some(Box::new(crate::plugins::core::Core::default())),
                Some(Box::new(crate::plugins::ha::Ha::default())),
                Some(Box::new(crate::plugins::teams::Teams::default())),
                Some(Box::new(crate::plugins::jetbrains::JetBrains::default())),
                Some(Box::new(crate::plugins::web::Web::default())),
            ],
            mqtt: None,
            mqtt_gen: 0,
            mqtt_up: false,
            avail_owner: String::new(),
            host: mqtt::host_name(),
            np: None,
            np_at: now,
            art: None,
            art_busy: false,
            profile_flash: None,
            dbg: None,
            steam_front: None,
            mail_seen: None,
            paired_seen: None,
            pomo_seen: None,
            mail_dismissed: None,
            announced: HashSet::new(),
            meeting_dismissed: None,
            game: None,
            game_cover: None,
            game_hidden: false,
            printer: None,
            status_capture: None,
            last_input: now,
            saver: None,
            scene: Scene::new(),
            swallow: [false; NUM_KEYS],
            pending_press: [None; NUM_KEYS],
            firmware: &[],
            fw_upgrade_tried: false,
            signals: Signals::default(),
            reads: vec![Reads::default(); NUM_KEYS],
        };
        e.restart_mqtt();
        e.configure_plugins();
        e
    }


    fn run(&mut self, rx: Receiver<Msg>) {
        loop {
            let now = Instant::now();
            let wait = self.next_deadline(now).saturating_duration_since(now);
            match rx.recv_timeout(wait) {
                Ok(m) => self.handle(m),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            self.timers();
        }
    }

    fn next_deadline(&self, now: Instant) -> Instant {
        let mut d = self.next_tick;
        if let Some(sv) = &self.saver {
            d = d.min(sv.next_frame);
        }
        if self.note.is_some() {
            d = d.min(self.next_frame);
        }
        let lp = Duration::from_millis(self.cfg.settings.long_press_ms.clamp(150, 5000));
        for h in self.held.iter().flatten() {
            if !h.long_fired {
                d = d.min(h.since + lp);
            }
        }
        for t in self.pending_press.iter().flatten() {
            d = d.min(*t);
        }
        d.max(now)
    }

    fn timers(&mut self) {
        let now = Instant::now();
        let lp = Duration::from_millis(self.cfg.settings.long_press_ms.clamp(150, 5000));
        for k in 0..NUM_KEYS {
            if let Some(h) = self.held[k] {
                if !h.long_fired && now >= h.since + lp {
                    self.held[k] = Some(Held { long_fired: true, ..h });
                    // a long second press: the first one was a press after all
                    if self.pending_press[k].take().is_some() {
                        self.fire(k, KeyEvent::Press);
                    }
                    self.fire(k, KeyEvent::Long);
                }
            }
            // no second press came in time: it was a single press
            if self.held[k].is_none() && self.pending_press[k].is_some_and(|t| now >= t) {
                self.pending_press[k] = None;
                self.fire(k, KeyEvent::Press);
            }
        }
        if self.saver.as_ref().is_some_and(|sv| now >= sv.next_frame) {
            self.saver_frame();
        }
        if self.note.is_some() && self.saver.is_none() && now >= self.next_frame {
            // smooth arrival, then a slow refresh for the countdown line
            let settled = self.note.as_ref().is_some_and(|(_, t0)| t0.elapsed() > Duration::from_millis(1300));
            self.next_frame = now + if settled { Duration::from_millis(500) } else { FRAME };
            self.draw_status();
        }
        if now >= self.next_tick {
            self.next_tick = now + Duration::from_secs(1);
            self.tick();
        }
    }

    // -------------------------------------------------------------- messages

    fn handle(&mut self, m: Msg) {
        match m {
            Msg::Dev(DevEvent::Key { dev, key, down }) if self.is_current(dev) && key < NUM_KEYS => {
                if down {
                    self.on_down(key)
                } else {
                    self.on_up(key)
                }
            }
            Msg::Dev(DevEvent::Closed { dev }) if self.is_current(dev) => {
                self.dev = None;
                self.saver = None;
                self.connected.store(false, Ordering::SeqCst);
                self.held = [None; NUM_KEYS];
                self.pending_press = [None; NUM_KEYS];
                self.set_status(|s| {
                    s.connected = false;
                    s.version = None;
                    s.message = "Connection lost, reconnecting…".into();
                });
                self.publish_availability(false);
            }
            Msg::Dev(_) => {}
            Msg::DevUp(dev) => self.on_connected(dev),
            Msg::Conn(st) => {
                if self.dev.is_none() {
                    self.set_status(|s| {
                        s.connected = false;
                        s.version = None;
                        s.adb = st.adb;
                        s.device_seen = st.device_seen;
                        s.message = st.message;
                    });
                }
            }
            Msg::Mqtt(gen, m) if gen == self.mqtt_gen => self.on_mqtt(m),
            Msg::Mqtt(..) => {}
            Msg::Ui(u) => self.on_ui(u),
            Msg::Error(e) => (self.emit)("error", json!({ "message": e })),
            Msg::Art(k, img) => {
                self.art_busy = false;
                self.art = Some((k, img.map(Arc::new)));
                self.draw_apps(Some(AppKind::Media));
                if self.note.is_none() {
                    self.draw_status();
                }
            }
            Msg::Plugin(id, m) => {
                self.call_plugin(id, |p, h, _| p.on_message(m, h));
                self.plugin_cards();
                self.emit_integrations();
            }
            Msg::SteamCover(id, img) => {
                self.game_cover = Some((id, img.map(Arc::new)));
            }
        }
    }

    fn is_current(&self, id: u64) -> bool {
        self.dev.as_ref().is_some_and(|d| d.id == id)
    }

    fn set_status(&mut self, f: impl FnOnce(&mut Status)) {
        let before = self.status.clone();
        f(&mut self.status);
        if self.status != before {
            (self.emit)("status", serde_json::to_value(&self.status).unwrap());
        }
    }

    fn on_connected(&mut self, dev: Arc<Device>) {
        let version = dev.version.clone();
        self.dev = Some(dev.clone());
        self.connected.store(true, Ordering::SeqCst);
        self.sent = [0; NUM_KEYS];
        let _ = dev.brightness(self.cfg.settings.brightness);
        let _ = dev.clear();
        self.draw_all();
        self.set_status(|s| {
            s.connected = true;
            s.device_seen = true;
            s.message = format!("Connected to d200fw {version}");
            s.version = Some(version.clone());
        });
        self.publish_availability(true);
        self.publish_state();
        self.maybe_upgrade_firmware(&version);
    }

    /// An older d200fw on the device: replace it with the embedded one and restart it.
    /// The connection drops and comes back with the new version.
    fn maybe_upgrade_firmware(&mut self, running: &str) {
        let embedded = env!("D200FW_VERSION");
        if self.fw_upgrade_tried || self.firmware.is_empty() || std::env::var_os("D200_DIRECT").is_some() || !crate::updater::newer(embedded, running) {
            return;
        }
        self.fw_upgrade_tried = true;
        let Some(adb) = device::adb_path() else { return };
        let (firmware, tx) = (self.firmware, self.tx.clone());
        self.set_status(|s| s.message = format!("Updating d200fw {running} to {embedded}..."));
        std::thread::spawn(move || {
            if let Err(e) = device::upgrade_firmware(&adb, firmware) {
                let _ = tx.send(Msg::Error(format!("d200fw update: {e}")));
            }
        });
    }

    // -------------------------------------------------------------- tick

    fn tick(&mut self) {
        self.ticks += 1;
        self.check_active_app();
        let ticks = self.ticks;
        for id in self.plugin_ids() {
            self.call_plugin(id, |p, h, c| p.tick(ticks, c, h));
        }
        self.plugin_cards();
        let app = self.cfg.status_key.app.clone();
        self.meeting_cards();
        if self.cfg.settings.cards.steam || app == "steam" {
            // A game comes to the front when it starts and leaves it when it ends: scan
            // when the front app changes. Without a front app (Linux), every 3 s.
            let front = self.status.active_app.as_ref().map(|a| a.id.clone());
            let due = if platform::front_app_supported() { front != self.steam_front } else { self.ticks % 3 == 1 };
            if due {
                self.steam_front = front;
                let game = self.dbg.get_or_insert_with(DebugWatch::new).steam_game();
                self.on_game(game);
            }
        }
        if self.ticks % 30 == 0 && self.dev.is_some() {
            self.publish_availability(true); // heals a stale retained "offline"
        }
        if self.rt_dirty && self.ticks % 2 == 0 {
            self.rt_dirty = false;
            if let Ok(s) = serde_json::to_string_pretty(&self.rt) {
                let _ = crate::config::write_private(&self.rt_path, &s);
            }
        }
        let ss = &self.cfg.settings.screensaver;
        if ss.enabled
            && self.saver.is_none()
            && self.dev.is_some()
            && self.note.is_none()
            && self.held.iter().all(Option::is_none)
            && self.last_input.elapsed() >= Duration::from_secs(ss.idle_minutes.max(1) as u64 * 60)
        {
            self.start_saver();
        }
        self.publish_core();
        if self.saver.is_some() {
            return; // the scene owns the screen
        }
        self.redraw_changed();
        // Dynamic faces; unchanged images are not re-sent.
        self.draw_apps(None);
        if self.note.is_none() {
            self.draw_status();
        }
    }

    /// Redraw keys whose icons read a signal that changed (built-in widgets redraw every tick).
    fn redraw_changed(&mut self) {
        let changed = self.signals.take_changed();
        if changed.is_empty() || self.saver.is_some() {
            return;
        }
        let v = self.view.clone();
        for k in 0..STATUS_KEY {
            if self.view_key(&v, k).kind().is_none() && self.reads[k].signals.iter().any(|s| changed.contains(s)) {
                self.draw_key(k);
            }
        }
        // built-in widgets that read plugin state (Teams, debugger), only when it changed
        let from = |ns: &str| changed.iter().any(|n| n.starts_with(ns));
        if from("com.deckling.teams:") {
            for kind in [AppKind::TeamsMute, AppKind::TeamsCamera, AppKind::TeamsPresence] {
                self.draw_apps(Some(kind));
            }
        }
        if from("com.deckling.jetbrains:") {
            self.draw_apps(Some(AppKind::Debugger));
        }
        if from("com.deckling.core:audio.") {
            self.draw_apps(Some(AppKind::Mute));
        }
        if from("com.deckling.core:weather.") {
            self.draw_apps(Some(AppKind::Weather));
        }
        if from("com.deckling.core:pomodoro.") {
            self.draw_apps(Some(AppKind::Pomodoro));
        }
        if from("com.deckling.core:media.") {
            self.on_media();
        }
    }

    /// The key shown at `k` in view `v`. On the remote page that is the Home Assistant
    /// entity `remote-<k>`, once Home Assistant has described it.
    fn view_key(&self, v: &View, k: usize) -> Key {
        if !v.remote {
            return self.cfg.key(v.profile, &v.page, k);
        }
        let id = format!("remote-{k}");
        if self.signals.get(&format!("{}:{id}.present", crate::plugins::ha::ID)).is_none() {
            return Key::default();
        }
        let mut key = Key { app: Some(format!("{}:entity", crate::plugins::ha::ID)), ..Default::default() };
        key.config.insert("id".into(), json!(id));
        key
    }

    /// The core's own signals: the clock. Plugins publish theirs (plugins.rs).
    fn publish_core(&mut self) {
        use chrono::Timelike;
        let now = chrono::Local::now();
        let (h, m, sec) = (now.hour() as f64, now.minute() as f64, now.second() as f64);
        let out: [(&str, Value); 7] = [
            ("time.clock", json!(now.format("%H:%M").to_string())),
            ("time.second", json!(sec)),
            ("time.tick", json!(self.ticks)),
            ("time.hour_angle", json!((h % 12.0) * 30.0 + m * 0.5)),
            ("time.minute_angle", json!(m * 6.0)),
            ("time.second_angle", json!(sec * 6.0)),
            ("time.second_progress", json!((sec * 100.0 / 60.0).round())),
        ];
        for (name, v) in out {
            self.signals.publish(name, v);
        }
    }

    fn check_active_app(&mut self) {
        let app = platform::frontmost_app();
        if app == self.status.active_app {
            return;
        }
        self.set_status(|s| s.active_app = app.clone());
        let Some(app) = app else { return };
        if OWN_APP_IDS.iter().any(|id| app.id == *id || app.name == *id) {
            return; // editing in this app should not switch profiles
        }
        if !self.cfg.settings.follow_active_app {
            return;
        }
        self.manual = false;
        let target = self.cfg.profiles.iter().position(|p| p.apps.iter().any(|e| app.matches(e))).unwrap_or(0);
        if target != self.view.profile {
            self.switch_profile(target);
        }
    }

    // -------------------------------------------------------------- keys

    fn on_down(&mut self, k: usize) {
        self.last_input = Instant::now();
        if k == STATUS_KEY && self.saver.is_none() {
            self.status_capture = Some(self.visible());
        }
        if self.saver.is_some() {
            // Waking does not run the key's action; a fresh press is needed.
            self.swallow[k] = true;
            self.stop_saver();
            return;
        }
        self.held[k] = Some(Held { since: Instant::now(), long_fired: false });
        (self.emit)("devkey", json!({ "key": k, "down": true }));
        self.draw_key(k);
        self.fire(k, KeyEvent::Down);
    }

    fn on_up(&mut self, k: usize) {
        self.last_input = Instant::now();
        if std::mem::take(&mut self.swallow[k]) {
            return;
        }
        let h = self.held[k].take();
        (self.emit)("devkey", json!({ "key": k, "down": false }));
        self.draw_key(k);
        self.fire(k, KeyEvent::Up);
        if h.is_some_and(|h| !h.long_fired) {
            if self.pending_press[k].take().is_some() {
                self.fire(k, KeyEvent::Double);
            } else if self.has_double(k) {
                // wait for a second press; timers() fires the press if none comes
                let wait = Duration::from_millis(self.cfg.settings.double_press_ms.clamp(100, 1000));
                self.pending_press[k] = Some(Instant::now() + wait);
            } else {
                self.fire(k, KeyEvent::Press);
            }
        }
        if k == STATUS_KEY {
            self.status_capture = None;
        }
    }

    /// Does key k have double-press actions right now? Only those wait before a press.
    fn has_double(&self, k: usize) -> bool {
        if k == STATUS_KEY {
            return !self.cfg.status_key.on.double.is_empty();
        }
        if self.view.remote {
            return false;
        }
        let key = self.key_cfg(k);
        let cur = self.rt.states.get(&self.key_path(k)).copied().unwrap_or(0);
        !key.state(cur).on.double.is_empty()
    }

    fn key_cfg(&self, k: usize) -> Key {
        self.cfg.key(self.view.profile, &self.view.page, k)
    }

    fn key_path(&self, k: usize) -> String {
        let p = self.cfg.profiles.get(self.view.profile).map(|p| p.name.as_str()).unwrap_or("");
        format!("{p}/{}/{k}", self.view.page)
    }

    fn fire(&mut self, k: usize, ev: KeyEvent) {
        if let Some(m) = &self.mqtt {
            let profile = self.cfg.profiles.get(self.view.profile).map(|p| p.name.clone()).unwrap_or_default();
            let page = if self.view.remote { "remote".to_string() } else { self.view.page.clone() };
            m.publish(&m.topic("key"), json!({ "profile": profile, "page": page, "key": k, "event": ev.name() }).to_string(), false);
            if self.view.remote && k != STATUS_KEY {
                m.publish(&m.topic(&format!("remote/{k}/action")), ev.name(), false);
            }
        }
        if k == STATUS_KEY {
            let vis = self.status_capture.clone().unwrap_or_else(|| self.visible());
            if self.status_press(vis, ev) {
                return;
            }
            let actions = self.cfg.status_key.on.get(ev).to_vec();
            for a in actions {
                self.run_ref(&a, None);
            }
            return;
        }
        if self.view.remote {
            return; // remote keys are handled by Home Assistant
        }
        let key = self.key_cfg(k);
        if let Some(kind) = key.kind() {
            self.app_action(k, kind, ev);
        }
        let path = self.key_path(k);
        let cur = self.rt.states.get(&path).copied().unwrap_or(0);
        let actions = key.state(cur).on.get(ev).to_vec();
        for a in actions {
            self.run_ref(&a, Some(k));
        }
    }

    /// Run a stored action: a core action, or report the missing plugin.
    fn run_ref(&mut self, r: &crate::config::ActionRef, key: Option<usize>) {
        let config = key.map(|k| self.view_key(&self.view.clone(), k).config).unwrap_or_default();
        let r = crate::config::ActionRef { params: crate::plugins::fill_params(&r.params, &config), ..r.clone() };
        if let Some(a) = r.parse() {
            return self.run_action(&a, key);
        }
        let (plugin, name) = r.action.split_once(':').unwrap_or(("core", ""));
        let res = self.call_plugin(plugin, |p, h, _| p.on_action(name, &r.params, h)).flatten();
        let msg = match res {
            Some(Ok(())) => return,
            Some(Err(e)) => e,
            None if !self.plugin_enabled(plugin) => format!("{}: plugin {plugin} is switched off", r.action),
            None if self.plugins.iter().flatten().any(|p| p.id() == plugin) || plugin == "core" => format!("bad action {}", r.action),
            None => format!("{}: plugin {plugin} is not installed", r.action),
        };
        (self.emit)("error", json!({ "message": msg }));
    }

    fn app_action(&mut self, k: usize, kind: AppKind, ev: KeyEvent) {
        let path = self.key_path(k);
        match (kind, ev) {
            (AppKind::Sysmon, KeyEvent::Press) | (AppKind::Clock, KeyEvent::Press) => {
                *self.rt.modes.entry(path).or_insert(0) += 1;
                self.rt_dirty = true;
                self.draw_key(k);
            }
            (AppKind::Mute, KeyEvent::Press) => self.run_plugin_action("com.deckling.core:mute", json!({})),
            (AppKind::Weather, KeyEvent::Press) => self.run_plugin_action("com.deckling.core:weather_refresh", json!({})),
            (AppKind::Pomodoro, KeyEvent::Press) => self.run_plugin_action("com.deckling.core:pomodoro", json!({})),
            (AppKind::Pomodoro, KeyEvent::Long) => self.run_plugin_action("com.deckling.core:pomodoro", json!({ "do": "reset" })),
            (AppKind::Media, KeyEvent::Press) => self.run_plugin_action("com.deckling.core:media", json!({ "command": "playpause" })),
            (AppKind::Media, KeyEvent::Long) => self.run_plugin_action("com.deckling.core:media", json!({ "command": "next" })),
            (AppKind::TeamsMute, KeyEvent::Press) => self.run_plugin_action("com.deckling.teams:meeting", json!({ "command": "toggle-mute" })),
            (AppKind::TeamsCamera, KeyEvent::Press) => self.run_plugin_action("com.deckling.teams:meeting", json!({ "command": "toggle-video" })),
            (AppKind::TeamsPresence, KeyEvent::Press) => self.run_plugin_action("com.deckling.teams:presence", json!({ "to": "toggle" })),
            (AppKind::TeamsPresence, KeyEvent::Long) => self.run_plugin_action("com.deckling.teams:presence", json!({ "to": "auto" })),
            (AppKind::Debugger, KeyEvent::Press) | (AppKind::Debugger, KeyEvent::Long) => {
                let key = self.key_cfg(k);
                let pick = |name: &str, def: &str| key.cfg_str(name).unwrap_or(def).to_string();
                let combo = match (crate::plugins::jetbrains::active(&self.signals).is_some(), ev) {
                    (true, KeyEvent::Press) => pick("active_keys", "cmd+alt+r"),
                    (true, _) => pick("stop_keys", "cmd+f2"),
                    (false, KeyEvent::Press) => pick("idle_keys", "ctrl+d"),
                    (false, _) => return,
                };
                self.run_plugin_action("com.deckling.core:keys", json!({ "combo": combo }));
            }
            _ => {}
        }
    }

    fn run_action(&mut self, a: &Action, key: Option<usize>) {
        let err = |e: String| (self.emit)("error", json!({ "message": e }));
        match a {
            Action::Page { page } => {
                let exists = self.cfg.profiles[self.view.profile].pages.contains_key(page);
                if exists && (page != &self.view.page || self.view.remote) {
                    if !self.view.remote {
                        self.stack.push(self.view.page.clone());
                    }
                    self.set_view(View { profile: self.view.profile, page: page.clone(), remote: false });
                }
            }
            Action::Back => {
                if self.view.remote {
                    self.set_view(View { remote: false, ..self.view.clone() });
                } else if let Some(p) = self.stack.pop() {
                    self.set_view(View { page: p, ..self.view.clone() });
                }
            }
            Action::Home => {
                self.stack.clear();
                let first = self.cfg.profiles[self.view.profile].pages.keys().next().cloned().unwrap_or_default();
                self.set_view(View { profile: self.view.profile, page: first, remote: false });
            }
            Action::Profile { profile } => match profile.as_str() {
                "next" => {
                    self.manual = true;
                    self.switch_profile((self.view.profile + 1) % self.cfg.profiles.len());
                }
                "auto" => {
                    self.manual = false;
                    self.status.active_app = None; // re-evaluate on the next tick
                }
                name => match self.cfg.profile_index(name) {
                    Some(i) => {
                        self.manual = true;
                        self.switch_profile(i);
                    }
                    None => err(format!("no profile named '{name}'")),
                },
            },
            Action::State { to } => {
                let Some(k) = key else { return };
                let n = self.key_cfg(k).states.len().max(1);
                let path = self.key_path(k);
                let cur = self.rt.states.get(&path).copied().unwrap_or(0);
                let next = if to == "next" { (cur + 1) % n } else { to.parse::<usize>().unwrap_or(0).min(n - 1) };
                self.rt.states.insert(path, next);
                self.rt_dirty = true;
                self.draw_key(k);
            }
            Action::Mqtt { topic, payload, retain } => match &self.mqtt {
                Some(m) if self.mqtt_up => m.publish(topic, payload.clone(), *retain),
                _ => err("MQTT is not connected".into()),
            },
            Action::Brightness { value } => {
                let cur = self.cfg.settings.brightness as i32;
                let v = value.trim();
                let new = if let Some(d) = v.strip_prefix('+') {
                    cur + d.parse::<i32>().unwrap_or(0)
                } else if v.starts_with('-') {
                    cur + v.parse::<i32>().unwrap_or(0)
                } else {
                    v.parse::<i32>().unwrap_or(cur)
                };
                self.set_brightness(new.clamp(5, 255) as u8, true);
            }
            Action::Notify { text, icon, color } => {
                self.notify(Notification::simple(text, icon.as_deref(), color.as_deref()));
            }
            Action::Remote => {
                if !self.view.remote {
                    self.set_view(View { remote: true, ..self.view.clone() });
                }
            }
        }
    }

    // -------------------------------------------------------------- media, Teams

    /// Track position, advanced between polls while playing.
    fn np_now(&self) -> Option<NowPlaying> {
        let mut np = self.np.clone()?;
        if np.playing {
            np.position = (np.position + self.np_at.elapsed().as_secs_f64()).min(np.duration.max(np.position));
        }
        Some(np)
    }

    fn integrations(&self) -> Value {
        let mut v = json!({
            "game": self.game.as_ref().map(|g| g.name.clone()),
            "printer": self.printer.as_ref().map(|p| p.state.clone()),
            "debugging": crate::plugins::jetbrains::active(&self.signals),
        });
        // what the plugins report about their connections
        for p in self.plugins.iter().flatten() {
            if let (Value::Object(st), Some(out)) = (p.status(), v.as_object_mut()) {
                out.extend(st);
            }
        }
        v
    }

    /// Run a plugin action from built-in app behaviour (a press on a Teams key).
    fn run_plugin_action(&mut self, id: &str, params: Value) {
        let r = crate::config::ActionRef { action: id.into(), params: params.as_object().cloned().unwrap_or_default(), ..Default::default() };
        self.run_ref(&r, None);
    }

    /// `plugins.<id>.enabled` in deck.json; plugins are on unless switched off.
    fn plugin_enabled(&self, id: &str) -> bool {
        self.cfg.plugins.get(id).is_none_or(|p| p.enabled)
    }

    fn plugin_ids(&self) -> Vec<&'static str> {
        self.plugins.iter().flatten().map(|p| p.id()).collect()
    }

    /// After a config change (and at start): every plugin starts or stops what it needs.
    fn configure_plugins(&mut self) {
        for id in self.plugin_ids() {
            if self.plugin_enabled(id) {
                self.call_plugin(id, |p, h, c| p.configure(c, h));
            } else {
                // a plugin switched off leaves nothing behind
                let prefix = format!("{id}:");
                let names: Vec<String> = self.signals.all().map(|(n, _)| n.clone()).filter(|n| n.starts_with(&prefix)).collect();
                for n in names {
                    self.signals.clear(&n);
                }
            }
        }
        self.redraw_changed();
        self.emit_integrations();
    }

    /// The player changed (com.deckling.core media signals): keep the track for the
    /// status key, fetch its artwork, redraw.
    fn on_media(&mut self) {
        let np = crate::plugins::core::now_playing(&self.signals);
        self.np_at = Instant::now();
        let key = np.as_ref().map(NowPlaying::track_key);
        if np != self.np {
            self.np = np;
            self.draw_apps(Some(AppKind::Media));
            if self.note.is_none() {
                self.draw_status();
            }
        }
        if let (Some(k), Some(np)) = (key, self.np.clone()) {
            if self.art.as_ref().is_none_or(|(ak, _)| *ak != k) && !self.art_busy {
                self.art_busy = true;
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let img = media::artwork(&np).and_then(|b| image::load_from_memory(&b).ok());
                    let _ = tx.send(Msg::Art(k, img));
                });
            }
        }
    }

    /// Cards that follow plugin signals: pairing with Teams, a new mail, Pomodoro phases.
    fn plugin_cards(&mut self) {
        let (n, last) = crate::plugins::core::pomodoro_events(&self.signals);
        if self.pomo_seen.is_some_and(|seen| n > seen) {
            let (text, icon, color) = match last.as_deref() {
                Some("long_break") => ("Long break!", "pomodoro-break", "#16a34a"),
                Some("break") => ("Time for a break", "pomodoro-break", "#16a34a"),
                _ => ("Break over, press to focus", "pomodoro-focus", "#dc2626"),
            };
            self.notify(Notification { title: Some("Pomodoro".into()), style: Some("flash".into()), ..Notification::simple(text, Some(icon), Some(color)) });
        }
        self.pomo_seen = Some(n);
        let paired = crate::plugins::teams::link_state(&self.signals).paired;
        if self.paired_seen == Some(false) && paired {
            self.notify(Notification::simple("Paired with Teams", Some("check"), Some("#4f46e5")));
        }
        self.paired_seen = Some(paired);
        if let Some(m) = crate::plugins::teams::mail(&self.signals) {
            let latest = m.latest.as_ref().map(|l| l.id.clone());
            let first = self.mail_seen.is_none();
            if !first && latest.is_some() && self.mail_seen.as_ref() != Some(&latest) && self.cfg.settings.cards.email {
                if let Some(c) = self.mail_card(&m, true) {
                    self.show_card(c);
                }
            }
            self.mail_seen = Some(latest);
        }
    }

    fn emit_integrations(&self) {
        (self.emit)("integrations", self.integrations());
    }

    fn set_brightness(&mut self, v: u8, save: bool) {
        self.cfg.settings.brightness = v;
        self.cfg.sync();
        if let Some(d) = &self.dev {
            let _ = d.brightness(v);
        }
        if save {
            let _ = self.cfg.save(&self.path);
            (self.emit)("config", json!({ "brightness": v }));
        }
        if let Some(m) = &self.mqtt {
            m.publish(&m.topic("brightness"), v.to_string(), true);
        }
    }

    fn switch_profile(&mut self, i: usize) {
        let first = self.cfg.profiles[i].pages.keys().next().cloned().unwrap_or_default();
        self.stack.clear();
        self.set_view(View { profile: i, page: first, remote: false });
    }

    fn set_view(&mut self, v: View) {
        if v == self.view {
            return;
        }
        if v.profile != self.view.profile && self.np.as_ref().is_some_and(|n| n.playing) {
            self.profile_flash = Some(Instant::now() + Duration::from_millis(2500));
        }
        self.view = v;
        self.held = [None; NUM_KEYS];
        self.pending_press = [None; NUM_KEYS];
        self.draw_all();
        (self.emit)("view", json!({ "profile": self.view.profile, "page": self.view.page, "remote": self.view.remote }));
        self.publish_state();
    }

    fn notify(&mut self, n: Notification) {
        self.show_card(Card::from(&n));
    }

    /// Queue a card on the wide display: higher priority interrupts, same group replaces.
    fn show_card(&mut self, card: Card) {
        if let Some(g) = card.group.clone() {
            self.notes.retain(|c| c.group.as_ref() != Some(&g));
            if let Some((cur, _)) = self.note.as_mut() {
                if cur.group.as_ref() == Some(&g) {
                    *cur = card;
                    self.next_frame = Instant::now();
                    return;
                }
            }
        }
        let critical = card.critical;
        let insert = |q: &mut VecDeque<Card>, c: Card| {
            let pos = q.iter().position(|x| x.priority() < c.priority()).unwrap_or(q.len());
            q.insert(pos, c);
            q.truncate(10);
        };
        if self.saver.is_some() {
            insert(&mut self.notes, card);
            if critical {
                self.stop_saver(); // urgent alerts interrupt the idle scene
            }
            return; // others wait behind a quiet dot
        }
        match self.note.take() {
            None => {
                insert(&mut self.notes, card);
                self.next_note();
            }
            Some((cur, t0)) => {
                if card.priority() > cur.priority() {
                    insert(&mut self.notes, cur);
                    self.note = Some((card, Instant::now()));
                    self.next_frame = Instant::now();
                } else {
                    self.note = Some((cur, t0));
                    insert(&mut self.notes, card);
                }
            }
        }
    }

    fn dismiss_note(&mut self) {
        if self.note.is_some() {
            self.next_note();
        }
    }

    // -------------------------------------------------------------- wide display content

    fn mail_card(&self, m: &teams::Mail, arriving: bool) -> Option<Card> {
        let hide = self.cfg.settings.cards.hide_content;
        let unread = format!("{} unread", m.unread);
        match &m.latest {
            Some(l) if m.unread > 0 => {
                if !arriving && self.mail_dismissed.as_ref() == Some(&l.id) {
                    return Some(Card { kind: Kind::Email, source: Some("Inbox".into()), title: unread, ..Default::default() });
                }
                Some(Card {
                    kind: Kind::Email,
                    source: Some("New mail".into()),
                    title: if hide { "New email".into() } else { l.from.clone() },
                    detail: if hide { None } else { Some(l.subject.clone()) },
                    note: Some(unread),
                    url: l.link.clone(),
                    group: Some("email".into()),
                    ..Default::default()
                })
            }
            _ => Some(Card { kind: Kind::Email, source: Some("Inbox".into()), title: "Inbox clear".into(), icon: Some("check".into()), ..Default::default() }),
        }
    }

    fn meeting_card(&self, e: &teams::CalEvent, now: i64) -> Card {
        let local = |t: i64| {
            chrono::DateTime::from_timestamp(t, 0)
                .map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string())
                .unwrap_or_default()
        };
        let delta = e.start - now;
        let title = if now >= e.start {
            if now < e.start + 120 { "Now".to_string() } else { "Started".to_string() }
        } else if delta < 3600 {
            format!("In {} min", (delta + 59) / 60)
        } else {
            format!("At {}", local(e.start))
        };
        let mut note = format!("{}–{}", local(e.start), local(e.end));
        if e.teams {
            note += " · Teams";
        }
        Card {
            kind: Kind::Meeting,
            source: Some("Next meeting".into()),
            title,
            detail: Some(e.subject.clone()),
            note: Some(note),
            url: e.join.clone(),
            group: Some(format!("meeting:{}", e.id)),
            ..Default::default()
        }
    }

    /// Reminder cards: 5 minutes before, and when it starts (once each).
    fn meeting_cards(&mut self) {
        if !self.cfg.settings.cards.meetings {
            return;
        }
        let now = chrono::Utc::now().timestamp();
        let due: Vec<(String, teams::CalEvent)> = crate::plugins::teams::events(&self.signals)
            .into_iter()
            .filter_map(|e| {
                let d = e.start - now;
                if d > 0 && d <= 300 {
                    Some((format!("{}:soon", e.id), e))
                } else if d <= 0 && d > -120 {
                    Some((format!("{}:live", e.id), e))
                } else {
                    None
                }
            })
            .collect();
        for (key, e) in due {
            if self.announced.insert(key) {
                let c = self.meeting_card(&e, now);
                self.show_card(c);
            }
        }
    }

    fn on_game(&mut self, game: Option<crate::plugins::jetbrains::ide::Game>) {
        if game.as_ref().map(|g| (&g.name, g.started)) == self.game.as_ref().map(|g| (&g.name, g.started)) {
            return;
        }
        let old = std::mem::replace(&mut self.game, game.clone());
        if let Some(g) = &game {
            if let Some(id) = g.appid {
                if self.game_cover.as_ref().is_none_or(|(c, _)| *c != id) {
                    let tx = self.tx.clone();
                    std::thread::spawn(move || {
                        let url = format!("https://cdn.akamai.steamstatic.com/steam/apps/{id}/header.jpg");
                        let img = ureq::get(&url).timeout(Duration::from_secs(8)).call().ok().and_then(|r| {
                            let mut buf = vec![];
                            std::io::Read::read_to_end(&mut r.into_reader(), &mut buf).ok()?;
                            image::load_from_memory(&buf).ok()
                        });
                        let _ = tx.send(Msg::SteamCover(id, img));
                    });
                }
            }
            if self.cfg.settings.cards.steam && self.cfg.status_key.app != "steam" {
                let mut c = self.game_card(g);
                c.detail = Some("Launched".into());
                self.show_card(c);
            }
        } else if let Some(g) = old {
            self.game_hidden = false;
            if self.cfg.settings.cards.steam {
                let mins = (now_secs().saturating_sub(g.started)) / 60;
                self.show_card(Card {
                    kind: Kind::Steam,
                    source: Some("Steam".into()),
                    title: g.name.clone(),
                    detail: Some(format!("Exited · {mins} min")),
                    group: Some("steam".into()),
                    ..Default::default()
                });
            }
        }
    }

    fn game_card(&self, g: &crate::plugins::jetbrains::ide::Game) -> Card {
        let mins = (now_secs().saturating_sub(g.started)) / 60;
        Card {
            kind: Kind::Steam,
            source: Some("Steam".into()),
            title: g.name.clone(),
            detail: Some(if mins >= 60 { format!("Playing · {}h {:02}m", mins / 60, mins % 60) } else { format!("Playing · {mins} min") }),
            url: g.appid.map(|id| format!("steam://nav/games/details/{id}")),
            group: Some("steam".into()),
            ..Default::default()
        }
    }

    /// The configured default content of the wide key, as a card (None = profile).
    fn default_card(&self) -> Option<Card> {
        let signed_out = || Card {
            kind: Kind::Meeting,
            source: Some("Microsoft 365".into()),
            title: "Sign in".into(),
            detail: Some("Settings → Teams".into()),
            icon: Some("outlook".into()),
            ..Default::default()
        };
        match self.cfg.status_key.app.as_str() {
            "media" => Some(match self.np_now() {
                Some(np) => Card {
                    kind: Kind::Media,
                    source: Some(if np.playing { "Playing".into() } else { "Paused".into() }),
                    title: np.title.clone(),
                    detail: Some(np.artist.clone()),
                    progress: (np.duration > 0.0).then(|| (np.position / np.duration) as f32),
                    note: (np.duration > 0.0).then(|| cards::mmss(np.position, np.duration)),
                    icon: Some(if np.player == "Spotify" { "spotify".into() } else { "now-playing".into() }),
                    color: (!np.playing).then(|| "#87909E".into()),
                    ..Default::default()
                },
                None => Card { kind: Kind::Media, source: Some("Media".into()), title: "Nothing playing".into(), icon: Some("now-playing".into()), ..Default::default() },
            }),
            "meeting" => {
                if !crate::plugins::teams::signed_in(&self.signals) {
                    return Some(signed_out());
                }
                let now = chrono::Utc::now().timestamp();
                let events = crate::plugins::teams::events(&self.signals);
                let next = events.iter().find(|e| e.end > now && self.meeting_dismissed.as_ref() != Some(&e.id));
                Some(match next {
                    Some(e) => self.meeting_card(e, now),
                    None => Card { kind: Kind::Meeting, source: Some("Calendar".into()), title: "No meetings".into(), detail: Some("Nothing in the next 12 hours".into()), ..Default::default() },
                })
            }
            "email" => {
                if !crate::plugins::teams::signed_in(&self.signals) {
                    return Some(signed_out());
                }
                let mail_err = self.signals.get("com.deckling.teams:mail.error").and_then(Value::as_str).map(str::to_string);
                match (crate::plugins::teams::mail(&self.signals), &mail_err) {
                    (Some(m), _) => self.mail_card(&m, false),
                    (None, Some(e)) => Some(Card { kind: Kind::Email, source: Some("Mail".into()), title: "Unavailable".into(), detail: Some(e.clone()), color: Some("#87909E".into()), ..Default::default() }),
                    _ => Some(Card { kind: Kind::Email, source: Some("Mail".into()), title: "—".into(), detail: Some("Loading".into()), ..Default::default() }),
                }
            }
            "steam" => Some(match (&self.game, self.game_hidden) {
                (Some(g), false) => self.game_card(g),
                _ => Card { kind: Kind::Steam, source: Some("Steam".into()), title: "Not playing".into(), icon: Some("steam-library".into()), color: Some("#87909E".into()), ..Default::default() },
            }),
            "printer" => Some(match &self.printer {
                Some(p) => p.card(),
                None => Card { kind: Kind::Printer, source: Some("Printer".into()), title: "—".into(), detail: Some("No data on MQTT".into()), icon: Some("print-offline".into()), color: Some("#87909E".into()), ..Default::default() },
            }),
            _ => None,
        }
    }

    fn visible(&self) -> Visible {
        if let Some((c, _)) = &self.note {
            return Visible::Note(c.clone());
        }
        if !self.view.remote && self.default_card().is_some() && !(self.cfg.status_key.app == "steam" && self.game_hidden) {
            return Visible::App(self.cfg.status_key.app.clone());
        }
        Visible::Profile
    }

    /// Press on the wide key: acts on what was visible when the press started.
    fn status_press(&mut self, vis: Visible, ev: KeyEvent) -> bool {
        let dir = base_dir(&self.path);
        match vis {
            Visible::Profile => return false,
            Visible::Note(card) => match ev {
                KeyEvent::Press => {
                    if !card.critical {
                        if let Some(u) = &card.url {
                            let _ = platform::open(u, &dir);
                        }
                    }
                    self.dismiss_note();
                }
                KeyEvent::Long => self.dismiss_note(),
                _ => {}
            },
            Visible::App(app) => match (app.as_str(), ev) {
                ("media", KeyEvent::Press) => self.run_plugin_action("com.deckling.core:media", json!({ "command": "playpause" })),
                ("media", KeyEvent::Long) => {
                    let player = self.np.as_ref().map(|n| n.player.clone()).unwrap_or_else(|| "Spotify".into());
                    let _ = platform::open(&player, &dir);
                }
                ("meeting", KeyEvent::Press) => {
                    if let Some(u) = self.default_card().and_then(|c| c.url) {
                        let _ = platform::open(&u, &dir);
                    }
                }
                ("meeting", KeyEvent::Long) => {
                    let now = chrono::Utc::now().timestamp();
                    self.meeting_dismissed = crate::plugins::teams::events(&self.signals).into_iter().find(|e| e.end > now).map(|e| e.id);
                }
                ("email", KeyEvent::Press) => {
                    let url = crate::plugins::teams::mail(&self.signals).and_then(|m| m.latest).and_then(|l| l.link);
                    let _ = platform::open(url.as_deref().unwrap_or("Microsoft Outlook"), &dir);
                }
                ("email", KeyEvent::Long) => {
                    self.mail_dismissed = crate::plugins::teams::mail(&self.signals).and_then(|m| m.latest).map(|l| l.id);
                }
                ("steam", KeyEvent::Press) => {
                    let url = self.game.as_ref().and_then(|g| g.appid).map(|id| format!("steam://nav/games/details/{id}"));
                    let _ = platform::open(url.as_deref().unwrap_or("steam://open/games"), &dir);
                }
                ("steam", KeyEvent::Long) => self.game_hidden = true,
                _ => {}
            },
        }
        self.draw_status();
        true
    }

    // -------------------------------------------------------------- screensaver

    fn start_saver(&mut self) {
        let ss = &self.cfg.settings.screensaver;
        let season = crate::screensaver::parse(&ss.scene, ss.southern);
        let Some(pm) = tiny_skia::Pixmap::new(1280, 720) else { return };
        // Newer d200fw draws the scene itself: no frames over USB. Older ones say ERR.
        let fps = ss.fps.clamp(1, 20);
        let on_device = self.dev.as_ref().is_some_and(|d| d.saver(season.name(), fps, ss.motion).is_ok());
        self.saver = Some(Saver { since: Instant::now(), season, next_frame: Instant::now(), t: 0.0, pm, on_device, dot: false });
        (self.emit)("screensaver", json!({ "on": true, "scene": season.name() }));
    }

    fn stop_saver(&mut self) {
        if let Some(sv) = self.saver.take() {
            if sv.on_device {
                if let Some(d) = &self.dev {
                    let _ = d.saver_off();
                }
            }
            self.sent = [0; NUM_KEYS];
            self.draw_all();
            (self.emit)("screensaver", json!({ "on": false }));
            if self.note.is_none() && !self.notes.is_empty() {
                self.next_note();
            }
        }
    }

    /// One frame: the scene, faded in over the key faces for the first 2.2 s.
    fn saver_frame(&mut self) {
        let Some(dev) = self.dev.clone() else {
            self.saver = None;
            return;
        };
        // On the device: only keep the "notification waiting" dot up to date.
        if self.saver.as_ref().is_some_and(|sv| sv.on_device) {
            let pending = !self.notes.is_empty();
            let sv = self.saver.as_mut().unwrap();
            sv.next_frame = Instant::now() + Duration::from_millis(250);
            if sv.dot != pending && dev.saver_dot(pending).is_ok() {
                sv.dot = pending;
            }
            return;
        }
        let ss = self.cfg.settings.screensaver.clone();
        let fps = ss.fps.clamp(1, 20) as u64;
        let (fade, pending) = {
            let sv = self.saver.as_mut().unwrap();
            let el = sv.since.elapsed().as_secs_f32();
            if ss.motion {
                sv.t = el;
            }
            sv.next_frame = Instant::now() + Duration::from_millis(1000 / fps);
            let fade = (el / 2.2).min(1.0);
            // A still scene only needs frames while fading in.
            if !ss.motion && fade >= 1.0 {
                sv.next_frame = Instant::now() + Duration::from_secs(3600);
            }
            (fade, !self.notes.is_empty())
        };
        {
            let sv = self.saver.as_mut().unwrap();
            let (season, t) = (sv.season, sv.t);
            self.scene.render(season, t, &mut sv.pm);
        }
        for k in 0..NUM_KEYS {
            let (x, y, w, h) = crate::screensaver::key_rect(k);
            let mut rgb = Vec::with_capacity(w * h * 3);
            {
                let sv = self.saver.as_ref().unwrap();
                let data = sv.pm.data();
                for row in y..y + h {
                    let o = (row * 1280 + x) * 4;
                    for px in data[o..o + w * 4].chunks_exact(4) {
                        rgb.extend_from_slice(&px[..3]);
                    }
                }
            }
            if k == STATUS_KEY && pending {
                // quiet indicator for deferred notifications
                for yy in 14..26 {
                    for xx in (w - 26)..(w - 14) {
                        let (dx, dy) = (xx as f32 - (w as f32 - 20.0), yy as f32 - 20.0);
                        if dx * dx + dy * dy <= 36.0 {
                            let o = (yy * w + xx) * 3;
                            rgb[o..o + 3].copy_from_slice(&render::MINT);
                        }
                    }
                }
            }
            if fade < 1.0 {
                if let Some(face) = self.face(&self.view.clone(), k, false) {
                    // content fades a little faster (1.9 s) than the scene comes up (2.2 s)
                    let a = (fade * 2.2 / 1.9).min(1.0);
                    for (o, f) in rgb.iter_mut().zip(face.iter()) {
                        *o = (*f as f32 * (1.0 - a) + *o as f32 * a) as u8;
                    }
                }
            }
            let hsh = hash(&rgb);
            if self.sent[k] != hsh && dev.image(k, &rgb).is_ok() {
                self.sent[k] = hsh;
            }
        }
    }

    fn next_note(&mut self) {
        self.note = self.notes.pop_front().map(|n| (n, Instant::now()));
        self.next_frame = Instant::now();
        if self.note.is_none() {
            self.draw_status();
        }
    }

    // -------------------------------------------------------------- drawing

    fn fonts(&self) -> Option<&Fonts> {
        self.fonts.as_ref().ok()
    }

    /// The face of key `k` in view `v`.
    fn face(&self, v: &View, k: usize, pressed: bool) -> Option<Vec<u8>> {
        self.face_r(v, k, pressed).map(|(rgb, _)| rgb)
    }

    /// Theme variables: the palette as the core variables, then `theme.variables`.
    fn theme_vars(&self) -> Map<String, Value> {
        let t = &self.cfg.settings.theme;
        let p = &t.palette;
        let mut m = Map::new();
        for (k, v) in [
            ("background", p.background.as_str()),
            ("base", "#F4F1E8"),
            ("muted", "#87909E"),
            ("accent", p.active.as_str()),
            ("info", p.info.as_str()),
            ("warning", p.attention.as_str()),
            ("danger", p.error.as_str()),
        ] {
            m.insert(k.into(), json!(v));
        }
        let reg = crate::registry::current();
        // a full theme replaces the palette; overlays follow in order; the deck's own variables last
        let full = self.cfg.theme.as_deref().and_then(|id| reg.themes.get(id)).filter(|t| t.kind == "full");
        let overlays = self.cfg.overlays.iter().filter_map(|id| reg.themes.get(id)).filter(|t| t.kind == "partial");
        for th in full.into_iter().chain(overlays) {
            for (k, v) in &th.variables {
                m.insert(k.clone(), v.clone());
            }
        }
        for (k, v) in &t.variables {
            m.insert(k.clone(), v.clone());
        }
        m
    }

    /// The configured theme with the palette taken from the (possibly bound) variables,
    /// so built-in widgets follow them too.
    fn effective_theme(&self, vars: &Map<String, Value>) -> crate::config::Theme {
        let mut t = self.cfg.settings.theme.clone();
        if t.variables.is_empty() && self.cfg.theme.is_none() && self.cfg.overlays.is_empty() {
            return t;
        }
        let none = Map::new();
        let ctx = Ctx { implements: None, props: None, config: &none, signals: &self.signals, theme: vars };
        let get = |n: &str| bind::resolve(&json!({ "tone": n }), &ctx, &mut Reads::default()).map(|v| bind::text(&v));
        for (var, slot) in [
            ("accent", &mut t.palette.active),
            ("info", &mut t.palette.info),
            ("warning", &mut t.palette.attention),
            ("danger", &mut t.palette.error),
            ("background", &mut t.palette.background),
        ] {
            if let Some(v) = get(var).filter(|v| render::parse_color(v).is_some()) {
                *slot = v;
            }
        }
        t
    }

    /// The face of key `k`, and what its icon read.
    fn face_r(&self, v: &View, k: usize, pressed: bool) -> Option<(Vec<u8>, Reads)> {
        let f = self.fonts()?;
        let vars = self.theme_vars();
        let theme_eff = self.effective_theme(&vars);
        let mut reads = Reads::default();
        let base = base_dir(&self.path);
        let size = render::key_size(k);
        let mut c: Canvas = if k == STATUS_KEY {
            let media = match self.cfg.status_key.show.as_str() {
                "profile" => None,
                "media" => self.np_now(),
                _ => self.np_now().filter(|n| n.playing),
            }
            .filter(|_| !v.remote);
            let cover = |c: &Card| -> Option<&DynamicImage> {
                match c.kind {
                    Kind::Media => self.np.as_ref().and_then(|np| {
                        self.art.as_ref().filter(|(k, _)| *k == np.track_key()).and_then(|(_, a)| a.as_deref())
                    }),
                    Kind::Steam => {
                        let id = self.game.as_ref().and_then(|g| g.appid);
                        self.game_cover.as_ref().filter(|(i, _)| Some(*i) == id).and_then(|(_, a)| a.as_deref())
                    }
                    _ => None,
                }
            };
            let app_card = if v.remote { None } else { self.default_card().filter(|_| !(self.cfg.status_key.app == "steam" && self.game_hidden)) };
            let theme = &theme_eff;
            if let Some((n, t0)) = &self.note {
                cards::render(f, &base, n, &cards::Extra { cover: cover(n), th: cards::theme_for(theme, n) }, t0.elapsed().as_secs_f32())
            } else if let Some(card) = app_card {
                cards::render(f, &base, &card, &cards::Extra { cover: cover(&card), th: cards::theme_for(theme, &card) }, 99.0)
            } else {
                let p = self.cfg.profiles.get(v.profile);
                let art = media.as_ref().and_then(|np| {
                    self.art.as_ref().filter(|(k, _)| *k == np.track_key()).and_then(|(_, a)| a.as_deref())
                });
                render::status_face(
                    f,
                    &StatusInfo {
                        profile: p.map(|p| p.name.as_str()).unwrap_or(""),
                        page: &v.page,
                        auto: self.cfg.settings.follow_active_app && !self.manual,
                        remote: v.remote,
                        mqtt: self.cfg.settings.mqtt.enabled.then_some(self.mqtt_up),
                        app: self.status.active_app.as_ref().map(|a| a.name.as_str()),
                        media: media.as_ref().map(|np| (np, art)),
                    },
                )
            }
        } else {
            let key = self.view_key(v, k);
            let pname = self.cfg.profiles.get(v.profile).map(|p| p.name.as_str()).unwrap_or("");
            let path = format!("{pname}/{}/{k}", v.page);
            let reg = crate::registry::current();
            // An app of a switched-off plugin draws like one whose plugin is missing.
            let def = key.app.as_deref().filter(|id| self.plugin_enabled(crate::registry::namespace(id))).and_then(|id| reg.app(id));
            let st0 = key.state(0);
            let chosen = st0.look.icon.as_deref().map(str::trim).filter(|i| !i.is_empty());
            // Built-in widgets draw while the app's default icon is chosen.
            let builtin_face = def.is_some_and(|d| d.face.is_some() && chosen.is_none_or(|c| Some(c) == d.icon_static()));
            match key.kind() {
                Some(kind) if builtin_face => {
                    let st = st0.clone();
                    let default_bg = render::DEFAULT_BG;
                    let (bg, fg) = render::look_colors(&st.look, default_bg);
                    let mode = self.rt.modes.get(&path).copied().unwrap_or(0);
                    match kind {
                        AppKind::Sysmon => {
                            let modes = apps::sysmon_modes(&key);
                            let i = mode % modes.len();
                            let id = if modes[i] == "net" { "network" } else { modes[i] };
                            let mut th = render::Th::resolve(&theme_eff, id, if id == "ram" { render::Tone::Info } else { render::Tone::Active });
                            if st.look.bg.is_some() {
                                th.bg = bg;
                            }
                            let _ = fg;
                            apps::sysmon_face(f, &crate::plugins::core::sys(&self.signals), modes[i], i, modes.len(), &th)
                        }
                        AppKind::Mute => apps::mute_face(f, crate::plugins::core::muted(&self.signals), bg, fg),
                        AppKind::Weather => {
                            let (w, err, configured) = crate::plugins::core::weather(&self.signals);
                            apps::weather_face(f, w.as_ref(), err.as_deref(), configured, bg, fg)
                        }
                        AppKind::Clock => {
                            let mut th = render::Th::resolve(&theme_eff, "clock", render::Tone::Active);
                            if st.look.bg.is_some() {
                                th.bg = bg;
                            }
                            apps::clock_face(f, mode, key.cfg_bool("h12"), &th)
                        }
                        AppKind::Pomodoro => apps::pomodoro_face(f, &crate::plugins::core::pomodoro(&self.signals), &self.cfg.settings.pomodoro, bg, fg),
                        AppKind::Media => {
                            let np = self.np_now();
                            let art = np.as_ref().and_then(|n| {
                                self.art.as_ref().filter(|(k, _)| *k == n.track_key()).and_then(|(_, a)| a.as_deref())
                            });
                            apps::media_face(f, np.as_ref(), art, bg, fg)
                        }
                        AppKind::TeamsMute | AppKind::TeamsCamera => {
                            let which = if kind == AppKind::TeamsMute { "mute" } else { "camera" };
                            let s = &self.signals;
                            apps::teams_face(f, which, &crate::plugins::teams::link_state(s), crate::plugins::teams::meeting(s).as_ref(), bg)
                        }
                        AppKind::TeamsPresence => apps::presence_face(
                            f,
                            crate::plugins::teams::presence(&self.signals).as_ref(),
                            crate::plugins::teams::signed_in(&self.signals),
                            crate::plugins::teams::presence_error(&self.signals).as_deref(),
                            bg,
                        ),
                        AppKind::Debugger => apps::debugger_face(f, crate::plugins::jetbrains::active(&self.signals).as_deref(), bg, fg),
                    }
                }
                _ if key.app.is_some() && def.is_none() => render::missing_plugin(f, key.app.as_deref().unwrap_or(""), size),
                _ if key.app.is_some() => {
                    // Any icon the user picked, fed by the app's interfaces.
                    let d = def.expect("checked above");
                    let env = IconEnv::new(Ctx { implements: Some(&d.implements), props: Some(&st0.look.props), config: &key.config, signals: &self.signals, theme: &vars });
                    // The app may pick icon, text, value and background per key; the key's own wins.
                    let pick = |b: &Option<Value>| {
                        b.as_ref().and_then(|b| bind::resolve(b, &env.ctx, &mut env.reads.borrow_mut())).map(|v| bind::text(&v)).filter(|t| !t.is_empty())
                    };
                    let mut look = st0.look.clone();
                    if chosen.is_none() {
                        look.icon = pick(&d.icon);
                    }
                    if look.label.is_none() {
                        look.label = pick(&d.label);
                    }
                    if look.bg.is_none() {
                        look.bg = pick(&d.background).filter(|c| render::parse_color(c).is_some());
                    }
                    let value = pick(&d.value);
                    let c = render::button(f, &base, &look, value.as_deref(), size, render::DEFAULT_BG, Some(&env));
                    reads = env.reads.into_inner();
                    c
                }
                _ if key.is_empty() => Canvas::new(size.0, size.1, [0, 0, 0]),
                _ => {
                    let cur = self.rt.states.get(&path).copied().unwrap_or(0);
                    let look = key.state(cur).look;
                    let env = IconEnv::new(Ctx { implements: None, props: Some(&look.props), config: &key.config, signals: &self.signals, theme: &vars });
                    let c = render::button(f, &base, &look, None, size, render::DEFAULT_BG, Some(&env));
                    reads = env.reads.into_inner();
                    c
                }
            }
        };
        if pressed {
            let (w, h) = (c.w(), c.h());
            c.overlay(render::MINT, 0.10);
            c.stroke_rect(2.0, 2.0, w - 4.0, h - 4.0, 12.0, 5.0, render::MINT);
        }
        Some((c.into_rgb(), reads))
    }

    fn draw_key(&mut self, k: usize) {
        // The scene owns the screen; a key image would also end the device's screensaver.
        if self.saver.is_some() {
            return;
        }
        let Some(dev) = self.dev.clone() else { return };
        let pressed = self.held[k].is_some();
        let Some((rgb, reads)) = self.face_r(&self.view.clone(), k, pressed) else { return };
        self.reads[k] = reads;
        let h = hash(&rgb);
        if self.sent[k] == h {
            return;
        }
        // Only the changed rectangle goes over USB (a clock's seconds bar is a few hundred
        // bytes, not the whole 115 KB key), unless the device's copy is unknown.
        let (w, _) = render::key_size(k);
        let rect = dev.rects.get(k).copied().filter(|_| self.sent[k] != 0 && self.last[k].len() == rgb.len());
        let res = match (rect, rect.and_then(|_| changed_box(&self.last[k], &rgb, w as usize))) {
            (Some([x, y, ..]), Some((bx, by, bw, bh))) if bw * bh * 2 < rgb.len() / 3 => {
                let mut part = Vec::with_capacity(bw * bh * 3);
                for row in by..by + bh {
                    let o = (row * w as usize + bx) * 3;
                    part.extend_from_slice(&rgb[o..o + bw * 3]);
                }
                if dev.rle() {
                    dev.blit_rle(x + bx, y + by, bw, bh, &device::rle_encode(&part))
                } else {
                    dev.blit(x + bx, y + by, bw, bh, &part)
                }
            }
            _ if dev.rle() => dev.image_rle(k, &device::rle_encode(&rgb)),
            _ => dev.image(k, &rgb),
        };
        match res {
            Ok(()) => {
                self.sent[k] = h;
                self.last[k] = rgb;
                (self.emit)("face", json!({ "key": k }));
            }
            Err(e) => eprintln!("draw key {k}: {e}"),
        }
    }

    fn draw_all(&mut self) {
        for k in 0..NUM_KEYS {
            self.draw_key(k);
        }
    }

    fn draw_status(&mut self) {
        if let Some((n, t0)) = &self.note {
            if t0.elapsed().as_secs_f32() >= n.duration() {
                self.next_note();
                return;
            }
        }
        self.draw_key(STATUS_KEY);
    }

    /// Redraw app keys (all, or one kind) on the current page.
    fn draw_apps(&mut self, kind: Option<AppKind>) {
        if self.view.remote {
            return;
        }
        for k in 0..STATUS_KEY {
            if let Some(app) = self.key_cfg(k).kind() {
                if kind.is_none_or(|kd| kd == app) {
                    self.draw_key(k);
                }
            }
        }
    }

    // -------------------------------------------------------------- MQTT

    fn restart_mqtt(&mut self) {
        if let Some(m) = self.mqtt.take() {
            m.stop();
        }
        self.mqtt_gen += 1;
        self.mqtt_up = false;
        let cfg = self.cfg.settings.mqtt.clone();
        if !cfg.enabled || cfg.host.trim().is_empty() {
            self.set_status(|s| {
                s.mqtt_ok = None;
                s.mqtt = "off".into();
            });
            return;
        }
        let tx = self.tx.clone();
        let gen = self.mqtt_gen;
        self.mqtt = Some(Mqtt::start(&cfg, move |m| {
            let _ = tx.send(Msg::Mqtt(gen, m));
        }));
        let host = cfg.host.clone();
        self.set_status(|s| {
            s.mqtt_ok = Some(false);
            s.mqtt = format!("connecting to {host}…");
        });
    }

    fn on_mqtt(&mut self, m: MqttMsg) {
        match m {
            MqttMsg::Connected => {
                self.mqtt_up = true;
                let host = self.cfg.settings.mqtt.host.clone();
                self.set_status(|s| {
                    s.mqtt_ok = Some(true);
                    s.mqtt = format!("connected to {host}");
                });
                if let Some(mq) = &self.mqtt {
                    mq.subscribe_all();
                    for p in self.plugins.iter().flatten() {
                        for t in p.mqtt_topics() {
                            mq.subscribe(t);
                        }
                    }
                }
                self.publish_discovery();
                self.publish_availability(self.dev.is_some());
                self.publish_state();
                self.draw_status();
            }
            MqttMsg::Disconnected(e) => {
                self.mqtt_up = false;
                self.set_status(|s| {
                    s.mqtt_ok = Some(false);
                    s.mqtt = format!("MQTT: {e}");
                });
                self.draw_status();
            }
            MqttMsg::Message { topic, payload } => self.on_mqtt_message(&topic, &payload),
        }
    }

    fn on_mqtt_message(&mut self, topic: &str, payload: &[u8]) {
        let Some(base) = self.mqtt.as_ref().map(|m| m.base.clone()) else { return };
        let text = String::from_utf8_lossy(payload).trim().to_string();
        if topic == "homeassistant/status" {
            if text == "online" {
                self.publish_discovery();
                self.publish_state();
            }
            return;
        }
        let Some(rest) = topic.strip_prefix(&format!("{base}/")) else { return };
        match rest {
            "availability" => self.avail_owner = text,
            "notify" => {
                self.notify_text(&text);
            }
            "remote/show" => {
                let on = !matches!(text.to_ascii_lowercase().as_str(), "off" | "0" | "false" | "hide");
                if on != self.view.remote {
                    self.set_view(View { remote: on, ..self.view.clone() });
                }
            }
            "profile/set" => {
                if let Some(i) = self.cfg.profile_index(&text) {
                    self.manual = true;
                    self.switch_profile(i);
                } else if text == "auto" {
                    self.manual = false;
                    self.status.active_app = None;
                }
            }
            "brightness/set" => {
                if let Ok(v) = text.parse::<f64>() {
                    self.set_brightness(v.clamp(0.0, 255.0) as u8, true);
                }
            }
            _ => {
                // Everything else belongs to the plugins that read it.
                let ids: Vec<&'static str> = self
                    .plugins
                    .iter()
                    .flatten()
                    .filter(|p| p.mqtt_topics().iter().any(|t| crate::plugins::topic_matches(t, rest)))
                    .map(|p| p.id())
                    .collect();
                for id in ids {
                    self.call_plugin(id, |p, h, _| p.on_mqtt(rest, &text, h));
                }
                if rest == "printer" {
                    self.on_printer();
                }
                if let ["remote", k, "set"] = rest.split('/').collect::<Vec<_>>()[..] {
                    (self.emit)("remote", json!({ "key": k.parse::<usize>().unwrap_or(0) }));
                }
            }
        }
    }

    /// Run `f` on plugin `id` with a host bound to this engine, then send what it
    /// published over MQTT and redraw keys whose signals changed.
    fn call_plugin<R>(&mut self, id: &str, f: impl FnOnce(&mut dyn Plugin, &mut Host, &Config) -> R) -> Option<R> {
        if !self.plugin_enabled(id) {
            return None;
        }
        let i = self.plugins.iter().position(|p| p.as_ref().is_some_and(|p| p.id() == id))?;
        let mut p = self.plugins[i].take()?;
        let base = self.mqtt.as_ref().map(|m| m.base.clone()).unwrap_or_else(|| "d200".into());
        let (r, out, errors, reqs) = {
            let dir = base_dir(&self.path);
            let sender = crate::plugins::Sender::new(p.id(), self.tx.clone());
            let mut host = Host::new(p.id(), &mut self.signals, base).with_engine(dir, sender);
            host.front = self.status.active_app.clone();
            host.front_known = platform::front_app_supported();
            let r = f(p.as_mut(), &mut host, &self.cfg);
            (r, host.mqtt_out, host.errors, host.requests)
        };
        self.plugins[i] = Some(p);
        for e in errors {
            (self.emit)("error", json!({ "message": e }));
        }
        for (topic, payload, retain) in out {
            match &self.mqtt {
                Some(m) if self.mqtt_up => m.publish(&topic, payload, retain),
                _ => (self.emit)("error", json!({ "message": "MQTT is not connected" })),
            }
        }
        self.host_requests(reqs);
        self.redraw_changed();
        Some(r)
    }

    /// A notification as text or JSON (MQTT `notify`, the web API): plain text, a
    /// notification `{title, text, icon, ...}` or a full card `{kind, source, title, ...}`.
    /// False when it shows nothing.
    fn notify_text(&mut self, text: &str) -> bool {
        let v: Option<Value> = if text.trim_start().starts_with('{') { serde_json::from_str(text).ok() } else { None };
        let card = match v {
            // A full card: {"kind": "push", "source": ..., "title": ..., "detail": ...}
            Some(v) if v.get("kind").is_some() || v.get("source").is_some() || v.get("detail").is_some() => serde_json::from_value::<Card>(v).ok(),
            Some(v) => serde_json::from_value::<Notification>(v).ok().map(|n| Card::from(&n)),
            None => Some(Card::from(&Notification::simple(text, Some("notification"), None))),
        };
        match card.filter(|c| !c.title.is_empty() || c.source.is_some()) {
            Some(c) => {
                self.show_card(c);
                true
            }
            None => false,
        }
    }

    /// What a plugin asked the core to do during a call.
    fn host_requests(&mut self, reqs: Vec<crate::plugins::HostRequest>) {
        use crate::plugins::HostRequest;
        for r in reqs {
            match r {
                HostRequest::Action(a) => self.run_ref(&a, None),
                HostRequest::Notify(text) => {
                    self.notify_text(&text);
                }
                HostRequest::Press { key, event } => {
                    if key < NUM_KEYS {
                        self.fire(key, event);
                    }
                }
            }
        }
    }

    /// The printer card follows the printer signals of com.deckling.ha.
    fn on_printer(&mut self) {
        let g = |n: &str| self.signals.get(&format!("{}:printer.{n}", crate::plugins::ha::ID)).cloned();
        let text = |n: &str| g(n).and_then(|v| v.as_str().map(str::to_string));
        let Some(state) = text("state") else {
            self.printer = None;
            return;
        };
        let spools: Vec<Value> = (1..=4)
            .filter_map(|i| Some(json!({ "color": g(&format!("slot{i}_color"))?, "progress": g(&format!("slot{i}_remaining")).unwrap_or(json!(0)) })))
            .collect();
        let p = Printer {
            state,
            percent: g("progress").and_then(|v| v.as_f64()).map(|v| v as f32),
            remaining_min: g("remaining_min").and_then(|v| v.as_f64()).map(|v| v as f32),
            name: text("name"),
            job: text("job"),
            message: text("message"),
            spools: (!spools.is_empty()).then_some(spools),
            ams: text("ams"),
        };
        let old = self.printer.replace(p.clone());
        let st = p.state.to_ascii_lowercase();
        let changed = old.as_ref().map(|o| o.state.to_ascii_lowercase()) != Some(st.clone());
        if changed && self.cfg.settings.cards.printer && old.is_some() && matches!(st.as_str(), "complete" | "error") {
            self.show_card(p.card());
        }
        if self.note.is_none() {
            self.draw_status();
        }
    }

    fn publish_discovery(&self) {
        let Some(m) = &self.mqtt else { return };
        if !self.mqtt_up || !self.cfg.settings.mqtt.discovery {
            return;
        }
        let names: Vec<String> = self.cfg.profiles.iter().map(|p| p.name.clone()).collect();
        for (t, p) in m.discovery(&names) {
            m.publish(&t, p, true);
        }
    }

    fn publish_availability(&mut self, online: bool) {
        let Some(m) = &self.mqtt else { return };
        if !self.mqtt_up {
            return;
        }
        let mine = format!("online {}", self.host);
        if online {
            m.publish(&m.topic("availability"), mine.clone(), true);
            self.avail_owner = mine;
        } else if self.avail_owner == mine {
            // Only clear it if no other computer has taken over in the meantime.
            m.publish(&m.topic("availability"), "offline", true);
        }
    }

    fn publish_state(&self) {
        let Some(m) = &self.mqtt else { return };
        if !self.mqtt_up || self.dev.is_none() {
            return;
        }
        let profile = self.cfg.profiles.get(self.view.profile).map(|p| p.name.clone()).unwrap_or_default();
        m.publish(&m.topic("profile"), profile.clone(), true);
        m.publish(&m.topic("brightness"), self.cfg.settings.brightness.to_string(), true);
        m.publish(
            &m.topic("page"),
            json!({ "profile": profile, "page": self.view.page, "remote": self.view.remote }).to_string(),
            true,
        );
    }

    // -------------------------------------------------------------- UI

    fn snapshot(&self) -> Value {
        // Remote keys as Home Assistant described them (com.deckling.ha signals).
        let ha = |k: usize, f: &str| self.signals.get(&format!("{}:remote-{k}.{f}", crate::plugins::ha::ID)).cloned();
        let remote: HashMap<String, Value> = (0..STATUS_KEY)
            .filter(|k| ha(*k, "present").is_some())
            .map(|k| (k.to_string(), json!({ "look": { "label": ha(k, "label"), "icon": ha(k, "icon") }, "value": ha(k, "value") })))
            .collect();
        json!({
            "config": self.cfg,
            "view": { "profile": self.view.profile, "page": self.view.page, "remote": self.view.remote },
            "status": self.status,
            "configPath": self.path.display().to_string(),
            "fontError": self.fonts.as_ref().err(),
            "remote": remote,
            "manual": self.manual,
            "integrations": self.integrations(),
            "screensaver": self.saver.is_some(),
        })
    }

    fn on_ui(&mut self, u: Ui) {
        match u {
            Ui::Snapshot(r) => {
                let _ = r.send(self.snapshot());
            }
            Ui::Save(cfg, r) => {
                let res = self.apply_config(*cfg).map(|_| self.snapshot());
                let _ = r.send(res);
            }
            Ui::View { profile, page, remote } => {
                if profile < self.cfg.profiles.len() && (remote || self.cfg.profiles[profile].pages.contains_key(&page)) {
                    if profile != self.view.profile {
                        self.manual = true;
                    }
                    let page = if self.cfg.profiles[profile].pages.contains_key(&page) {
                        page
                    } else {
                        self.cfg.profiles[profile].pages.keys().next().cloned().unwrap_or_default()
                    };
                    self.stack.clear();
                    self.set_view(View { profile, page, remote });
                }
            }
            Ui::Preview { profile, page, remote, key, reply } => {
                let v = View { profile, page, remote };
                let res = if key >= NUM_KEYS {
                    Err("key out of range".to_string())
                } else {
                    match self.face(&v, key, false) {
                        Some(rgb) => {
                            let png = render::png(rgb, render::key_size(key));
                            Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
                        }
                        None => Err(self.fonts.as_ref().err().cloned().unwrap_or_default()),
                    }
                };
                let _ = reply.send(res);
            }
            Ui::Trigger { key, event } => {
                if key < NUM_KEYS {
                    self.fire(key, event);
                }
            }
            Ui::Notify(n) => self.notify(n),
            Ui::Options { source, profile, reply } => {
                let list = match source.as_str() {
                    "core:profiles" => [json!({ "label": "Next profile", "value": "next" }), json!({ "label": "Automatic", "value": "auto" })]
                        .into_iter()
                        .chain(self.cfg.profiles.iter().map(|p| json!({ "label": p.name, "value": p.name })))
                        .collect(),
                    "core:pages" => self
                        .cfg
                        .profiles
                        .get(profile)
                        .map(|p| p.pages.keys().map(|n| json!({ "label": n, "value": n })).collect())
                        .unwrap_or_default(),
                    other => other
                        .split_once(':')
                        .and_then(|(pl, name)| self.plugins.iter().flatten().find(|p| p.id() == pl).and_then(|p| p.options(name)))
                        .unwrap_or_default(),
                };
                let _ = reply.send(list);
            }
            Ui::Signals(reply) => {
                let mut all: Vec<(&String, &Value)> = self.signals.all().collect();
                all.sort_by(|a, b| a.0.cmp(b.0));
                let reg = crate::registry::current();
                let types = &reg.signals;
                let list: Vec<Value> = all
                    .into_iter()
                    .map(|(n, v)| json!({ "name": n, "value": v, "type": types.get(n.as_str()).map(|t| t.kind.as_str()) }))
                    .collect();
                let _ = reply.send(json!(list));
            }
            Ui::PreviewIcon { icon, profile, page, key, reply } => {
                let k = self.cfg.key(profile, &page, key);
                let reg = crate::registry::current();
                let def = k.app.as_deref().and_then(|id| reg.app(id));
                let vars = self.theme_vars();
                let st = k.state(0);
                let env = IconEnv::new(Ctx {
                    implements: def.map(|d| &d.implements),
                    props: Some(&st.look.props),
                    config: &k.config,
                    signals: &self.signals,
                    theme: &vars,
                });
                let res = match reg.icon(&icon) {
                    None => Err(format!("no icon {icon}")),
                    Some(d) => crate::icons::render_bound(d, 96, &[], &env.ctx, &mut Reads::default())
                        .map(|pm| {
                            let rgba = pm.data().to_vec();
                            let png = render::png_rgba(rgba, (96, 96));
                            format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
                        })
                        .ok_or_else(|| "could not draw".to_string()),
                };
                let _ = reply.send(res);
            }
            Ui::ReloadPackages(reply) => {
                let reg = crate::registry::load_disk(&base_dir(&self.path).join("packages"));
                self.draw_all();
                let _ = reply.send(reg.warnings.clone());
                (self.emit)("packages", json!(null));
            }
            Ui::Screensaver(on) => {
                if on {
                    self.start_saver();
                } else {
                    self.last_input = Instant::now();
                    self.stop_saver();
                }
            }
            Ui::GraphTokens(t) => {
                self.call_plugin(crate::plugins::teams::ID, |p, h, _| p.on_message(Box::new(crate::plugins::teams::TeamsMsg::SignedIn(t)), h));
                self.emit_integrations();
            }
        }
    }

    fn apply_config(&mut self, cfg: Config) -> Result<(), String> {
        cfg.validate()?;
        cfg.save(&self.path)?;
        let old = std::mem::replace(&mut self.cfg, cfg);
        if self.view.profile >= self.cfg.profiles.len() {
            self.view.profile = 0;
        }
        if !self.cfg.profiles[self.view.profile].pages.contains_key(&self.view.page) {
            self.view.page = self.cfg.profiles[self.view.profile].pages.keys().next().cloned().unwrap_or_default();
            self.stack.clear();
        }
        if old.settings.mqtt != self.cfg.settings.mqtt {
            self.restart_mqtt();
        } else if old.profiles.iter().map(|p| &p.name).ne(self.cfg.profiles.iter().map(|p| &p.name)) {
            self.publish_discovery();
        }
        if old.settings.brightness != self.cfg.settings.brightness {
            self.set_brightness(self.cfg.settings.brightness, false);
        }
        self.configure_plugins();
        if old.settings.screensaver != self.cfg.settings.screensaver && self.saver.is_some() {
            self.saver = None;
            self.start_saver();
        }
        if old.settings.follow_active_app != self.cfg.settings.follow_active_app {
            self.status.active_app = None; // re-evaluate
        }
        self.draw_all();
        Ok(())
    }
}

fn try_connect(id: u64, events: Sender<DevEvent>) -> Result<Device, Status> {
    // D200_DIRECT=1 skips adb and connects to the port directly (simulator).
    let direct = std::env::var_os("D200_DIRECT").is_some();
    let adb = device::adb_path();
    let adb_s = adb.as_ref().map(|p| p.display().to_string());
    if !direct {
        let Some(adb_bin) = &adb else {
            return Err(Status { message: "adb not found. Install Android platform-tools (see README).".into(), ..Default::default() });
        };
        if !device::adb_device_present(adb_bin) {
            return Err(Status { adb: adb_s, message: "No D200 connected over USB.".into(), ..Default::default() });
        }
    }
    Device::connect(if direct { None } else { adb.as_deref() }, id, events).map_err(|e| Status {
        adb: adb_s,
        device_seen: !direct,
        message: if direct {
            format!("No d200fw on port {}: {e}", device::port())
        } else {
            "D200 found, but d200fw is not running on it. Use \"Start firmware\".".into()
        },
        ..Default::default()
    })
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod diff_tests {
    use super::changed_box;

    #[test]
    fn finds_the_changed_rectangle() {
        let w = 10;
        let a = vec![0u8; w * 6 * 3];
        let mut b = a.clone();
        assert_eq!(changed_box(&a, &b, w), None);
        b[(2 * w + 3) * 3] = 255; // (3, 2)
        b[(4 * w + 7) * 3 + 2] = 9; // (7, 4)
        assert_eq!(changed_box(&a, &b, w), Some((3, 2, 5, 3)));
    }
}
