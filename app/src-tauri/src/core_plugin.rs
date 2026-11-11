//! com.deckling.core: the apps and actions that ship with the app but are not fixed in
//! it: system monitor, mute and volume, weather, Pomodoro, media, commands, opening
//! things and shortcuts. Night Signal is its icon set.
//!
//! Like any plugin it only publishes signals. The core's built-in widgets read them back
//! with the readers at the bottom of this file.

use crate::apps::{self, Phase, PomoEvent, Pomodoro, SysStats, Weather};
use crate::bind::Signals;
use crate::config::{AppKind, Config, Location, PomodoroCfg};
use crate::media::{self, NowPlaying};
use crate::platform;
use crate::plugins::{Host, Plugin, PluginMsg};
use serde_json::{json, Map, Value};
use std::time::{Duration, Instant};

pub const ID: &str = "com.deckling.core";

/// Results of the plugin's own threads.
enum CoreMsg {
    Mute(Option<bool>),
    Weather(u64, Result<Weather, String>),
    Media(Option<NowPlaying>),
    Error(String),
}

pub struct Core {
    sys: Option<SysStats>,
    wants_sys: bool,
    muted: Option<bool>,
    wants_mute: bool,
    mute_busy: bool,
    location: Option<Location>,
    fahrenheit: bool,
    weather: Option<Weather>,
    weather_err: Option<String>,
    weather_at: Option<Instant>,
    weather_gen: u64,
    weather_busy: bool,
    pomo_cfg: PomodoroCfg,
    pomo: Pomodoro,
    /// Pomodoro phase ends so far; the core shows a card when it changes.
    pomo_events: u64,
    np: Option<NowPlaying>,
    wants_media: bool,
    media_busy: bool,
}

impl Default for Core {
    fn default() -> Self {
        Core {
            sys: None,
            wants_sys: false,
            muted: None,
            wants_mute: false,
            mute_busy: false,
            location: None,
            fahrenheit: false,
            weather: None,
            weather_err: None,
            weather_at: None,
            weather_gen: 0,
            weather_busy: false,
            pomo_cfg: PomodoroCfg::default(),
            pomo: Pomodoro::new(),
            pomo_events: 0,
            np: None,
            wants_media: false,
            media_busy: false,
        }
    }
}

impl Core {
    fn poll_mute(&mut self, host: &Host) {
        let Some(tx) = host.sender().filter(|_| !self.mute_busy) else { return };
        self.mute_busy = true;
        std::thread::spawn(move || tx.send(CoreMsg::Mute(platform::get_muted())));
    }

    fn fetch_weather(&mut self, force: bool, host: &Host) {
        let Some(loc) = self.location.clone() else { return };
        if self.weather_busy {
            return;
        }
        let stale = self.weather_at.is_none_or(|t| {
            let age = t.elapsed();
            age > Duration::from_secs(15 * 60) || (self.weather_err.is_some() && age > Duration::from_secs(60))
        });
        let Some(tx) = host.sender().filter(|_| force || stale) else { return };
        self.weather_busy = true;
        let (gen, f) = (self.weather_gen, self.fahrenheit);
        std::thread::spawn(move || tx.send(CoreMsg::Weather(gen, apps::fetch_weather(&loc, f))));
    }

    fn poll_media(&mut self, host: &Host, after: Option<(Option<String>, String)>) {
        let Some(tx) = host.sender() else { return };
        self.media_busy = true;
        std::thread::spawn(move || {
            if let Some((player, cmd)) = after {
                if let Err(e) = media::command(player.as_deref(), &cmd) {
                    tx.send(CoreMsg::Error(e));
                }
                std::thread::sleep(Duration::from_millis(350));
            }
            tx.send(CoreMsg::Media(media::poll()));
        });
    }

    /// Everything as signals.
    fn publish(&self, host: &mut Host) {
        let mut out: Vec<(&str, Option<Value>)> = vec![];
        if let Some(s) = &self.sys {
            let pct = |a: u64, b: u64| if b == 0 { 0.0 } else { (a as f64 * 100.0 / b as f64).round() };
            let hist = |h: &std::collections::VecDeque<f32>| json!(h.iter().map(|v| (*v as f64 * 10.0).round() / 10.0).collect::<Vec<_>>());
            out.extend([
                ("system.cpu", Some(json!((s.cpu as f64).round()))),
                ("system.ram", Some(json!(pct(s.ram_used, s.ram_total)))),
                ("system.disk", Some(json!(pct(s.disk_used, s.disk_total)))),
                ("system.net_down", Some(json!(s.rx.round()))),
                ("system.net_up", Some(json!(s.tx.round()))),
                ("system.ram_used", Some(json!(s.ram_used))),
                ("system.ram_total", Some(json!(s.ram_total))),
                ("system.disk_used", Some(json!(s.disk_used))),
                ("system.disk_total", Some(json!(s.disk_total))),
                ("system.disk_name", Some(json!(s.disk_name))),
                ("system.temperature", s.temp.map(|t| json!((t as f64).round()))),
                ("system.history.cpu", Some(hist(&s.h_cpu))),
                ("system.history.ram", Some(hist(&s.h_ram))),
                ("system.history.net_down", Some(hist(&s.h_rx))),
                ("system.history.net_up", Some(hist(&s.h_tx))),
            ]);
        }
        out.push(("audio.muted", self.muted.map(Value::from)));
        let w = self.weather.as_ref();
        out.extend([
            ("weather.configured", Some(json!(self.location.is_some()))),
            ("weather.temperature", w.map(|w| json!(w.temp.round()))),
            ("weather.summary", w.map(|w| json!(apps::weather_info(w.code, w.is_day).1))),
            ("weather.code", w.map(|w| json!(w.code))),
            ("weather.is_day", w.map(|w| json!(w.is_day))),
            ("weather.max", w.map(|w| json!(w.max))),
            ("weather.min", w.map(|w| json!(w.min))),
            ("weather.place", w.map(|w| json!(w.place))),
            ("weather.error", self.weather_err.clone().map(Value::from)),
        ]);
        let p = &self.pomo;
        let rem = p.remaining(&self.pomo_cfg);
        let len = Pomodoro::len(if p.phase == Phase::Idle { Phase::Work } else { p.phase }, &self.pomo_cfg);
        let secs = rem.as_secs() + u64::from(rem.subsec_millis() > 0);
        out.extend([
            ("pomodoro.running", Some(json!(p.running))),
            ("pomodoro.phase", Some(json!(format!("{:?}", p.phase).to_lowercase()))),
            ("pomodoro.done", Some(json!(p.done))),
            ("pomodoro.remaining", Some(json!(format!("{}:{:02}", secs / 60, secs % 60)))),
            ("pomodoro.remaining_ms", Some(json!(rem.as_millis() as u64))),
            ("pomodoro.progress", Some(json!(if p.phase == Phase::Idle { 0.0 } else { (100.0 - rem.as_secs_f64() * 100.0 / len.as_secs_f64()).round() }))),
            ("pomodoro.events", Some(json!(self.pomo_events))),
        ]);
        let np = self.np.as_ref();
        out.extend([
            ("media.player", np.map(|n| json!(n.player))),
            ("media.playing", np.map(|n| json!(n.playing))),
            ("media.title", np.map(|n| json!(n.title))),
            ("media.artist", np.map(|n| json!(n.artist))),
            ("media.album", np.map(|n| json!(n.album))),
            ("media.position", np.map(|n| json!(n.position))),
            ("media.duration", np.map(|n| json!(n.duration))),
            ("media.art_url", np.map(|n| json!(n.art_url))),
            ("media.progress", np.map(|n| json!(if n.duration > 0.0 { (n.position * 100.0 / n.duration).round() } else { 0.0 }))),
        ]);
        for (name, v) in out {
            match v {
                Some(v) => host.publish(name, v),
                None => host.clear(name),
            };
        }
    }
}

impl Plugin for Core {
    fn id(&self) -> &'static str {
        ID
    }

    fn configure(&mut self, cfg: &Config, host: &mut Host) {
        let s = &cfg.settings;
        self.wants_sys = cfg.uses(&[AppKind::Sysmon], |_| false);
        self.wants_mute = cfg.uses(&[AppKind::Mute], |_| false);
        self.wants_media = media::available() && (cfg.status_key.show != "profile" || cfg.uses(&[AppKind::Media], |_| false));
        if !self.wants_sys {
            self.sys = None;
        }
        if s.weather.location != self.location || s.weather.fahrenheit != self.fahrenheit {
            self.location = s.weather.location.clone();
            self.fahrenheit = s.weather.fahrenheit;
            self.weather_gen += 1;
            self.weather = None;
            self.weather_err = None;
            self.weather_at = None;
            self.weather_busy = false;
            self.fetch_weather(true, host);
        }
        self.pomo_cfg = s.pomodoro.clone();
        if self.wants_mute && self.muted.is_none() {
            self.poll_mute(host);
        }
        self.publish(host);
    }

    fn tick(&mut self, ticks: u64, _cfg: &Config, host: &mut Host) {
        if self.wants_sys {
            self.sys.get_or_insert_with(SysStats::new).refresh();
        }
        if let Some(ev) = self.pomo.tick(&self.pomo_cfg) {
            self.pomo_events += 1;
            host.publish("pomodoro.last_event", json!(match ev {
                PomoEvent::WorkDone { long_break: true } => "long_break",
                PomoEvent::WorkDone { long_break: false } => "break",
                PomoEvent::BreakDone => "focus",
            }));
            if self.pomo_cfg.sound {
                platform::chime();
            }
        }
        if ticks % 3 == 0 && self.wants_mute {
            self.poll_mute(host);
        }
        self.fetch_weather(false, host);
        if ticks % 2 == 0 && self.wants_media && !self.media_busy {
            self.poll_media(host, None);
        }
        self.publish(host);
    }

    fn on_message(&mut self, msg: PluginMsg, host: &mut Host) {
        let Ok(msg) = msg.downcast::<CoreMsg>() else { return };
        match *msg {
            CoreMsg::Mute(m) => {
                self.mute_busy = false;
                self.muted = m;
            }
            CoreMsg::Weather(gen, r) => {
                if gen != self.weather_gen {
                    return;
                }
                self.weather_busy = false;
                self.weather_at = Some(Instant::now());
                match r {
                    Ok(w) => {
                        self.weather = Some(w);
                        self.weather_err = None;
                    }
                    Err(e) => self.weather_err = Some(e),
                }
            }
            CoreMsg::Media(np) => {
                self.media_busy = false;
                self.np = np;
            }
            CoreMsg::Error(e) => host.error(e),
        }
        self.publish(host);
    }

    fn on_action(&mut self, name: &str, params: &Map<String, Value>, host: &mut Host) -> Option<Result<(), String>> {
        let p = |k: &str| params.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let tx = host.sender();
        let r = match name {
            "command" => {
                let cmd = p("command");
                if cmd.trim().is_empty() {
                    Ok(())
                } else {
                    platform::run_shell(&cmd, &host.dir).map_err(|e| format!("command failed to start: {e}"))
                }
            }
            "open" => platform::open(&p("target"), &host.dir).map_err(|e| format!("open failed: {e}")),
            "keys" => {
                let combo = p("combo");
                std::thread::spawn(move || {
                    if let (Err(e), Some(tx)) = (platform::send_keys(&combo), tx) {
                        tx.send(CoreMsg::Error(e));
                    }
                });
                Ok(())
            }
            "volume" => {
                let value = p("value");
                std::thread::spawn(move || {
                    let r = platform::change_volume(&value);
                    if let Some(tx) = tx {
                        if let Err(e) = r {
                            tx.send(CoreMsg::Error(e));
                        }
                        tx.send(CoreMsg::Mute(platform::get_muted()));
                    }
                });
                Ok(())
            }
            "mute" => {
                let target = !self.muted.unwrap_or(false);
                self.muted = Some(target); // optimistic
                std::thread::spawn(move || {
                    let r = platform::set_muted(target);
                    if let Some(tx) = tx {
                        if let Err(e) = r {
                            tx.send(CoreMsg::Error(e));
                        }
                        tx.send(CoreMsg::Mute(platform::get_muted()));
                    }
                });
                Ok(())
            }
            "media" => {
                let command = p("command");
                if command == "playpause" {
                    if let Some(np) = self.np.as_mut() {
                        np.playing = !np.playing; // optimistic, for a snappy key
                    }
                }
                let player = self.np.as_ref().map(|n| n.player.clone());
                self.poll_media(host, Some((player, command)));
                Ok(())
            }
            "weather_refresh" => {
                self.fetch_weather(true, host);
                Ok(())
            }
            "pomodoro" => {
                match p("do").as_str() {
                    "reset" => self.pomo.reset(),
                    _ => self.pomo.press(&self.pomo_cfg),
                }
                Ok(())
            }
            _ => return None,
        };
        self.publish(host);
        Some(r)
    }

    fn status(&self) -> Value {
        json!({ "playing": self.np.as_ref().map(|n| json!({ "player": n.player, "title": n.title, "artist": n.artist, "playing": n.playing })) })
    }
}

// ------------------------------------------------------------------ what the core reads

fn get<'a>(s: &'a Signals, name: &str) -> Option<&'a Value> {
    s.get(&format!("{ID}:{name}"))
}

fn num(s: &Signals, name: &str) -> Option<f64> {
    get(s, name).and_then(Value::as_f64)
}

fn text(s: &Signals, name: &str) -> Option<String> {
    get(s, name).and_then(Value::as_str).map(str::to_string)
}

/// The system monitor's numbers and history.
#[derive(Debug, Clone, Default)]
pub struct SysView {
    pub cpu: f32,
    pub ram_used: u64,
    pub ram_total: u64,
    pub disk_used: u64,
    pub disk_total: u64,
    pub disk_name: String,
    pub rx: f64,
    pub tx: f64,
    pub temp: Option<f32>,
    pub h_cpu: Vec<f32>,
    pub h_ram: Vec<f32>,
    pub h_rx: Vec<f32>,
    pub h_tx: Vec<f32>,
}

pub fn sys(s: &Signals) -> SysView {
    let u = |n: &str| get(s, n).and_then(Value::as_u64).unwrap_or(0);
    let h = |n: &str| {
        get(s, &format!("system.history.{n}")).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).map(|v| v as f32).collect()).unwrap_or_default()
    };
    SysView {
        cpu: num(s, "system.cpu").unwrap_or(0.0) as f32,
        ram_used: u("system.ram_used"),
        ram_total: u("system.ram_total"),
        disk_used: u("system.disk_used"),
        disk_total: u("system.disk_total"),
        disk_name: text(s, "system.disk_name").unwrap_or_default(),
        rx: num(s, "system.net_down").unwrap_or(0.0),
        tx: num(s, "system.net_up").unwrap_or(0.0),
        temp: num(s, "system.temperature").map(|t| t as f32),
        h_cpu: h("cpu"),
        h_ram: h("ram"),
        h_rx: h("net_down"),
        h_tx: h("net_up"),
    }
}

pub fn muted(s: &Signals) -> Option<bool> {
    get(s, "audio.muted").and_then(Value::as_bool)
}

/// (weather, error, a place is set)
pub fn weather(s: &Signals) -> (Option<Weather>, Option<String>, bool) {
    let w = num(s, "weather.temperature").map(|temp| Weather {
        temp,
        code: get(s, "weather.code").and_then(Value::as_i64).unwrap_or(0),
        is_day: get(s, "weather.is_day").and_then(Value::as_bool).unwrap_or(true),
        max: num(s, "weather.max").unwrap_or(temp),
        min: num(s, "weather.min").unwrap_or(temp),
        place: text(s, "weather.place").unwrap_or_default(),
    });
    let configured = get(s, "weather.configured").and_then(Value::as_bool).unwrap_or(false);
    (w, text(s, "weather.error"), configured)
}

pub fn pomodoro(s: &Signals) -> Pomodoro {
    let phase = match text(s, "pomodoro.phase").as_deref() {
        Some("work") => Phase::Work,
        Some("short") => Phase::Short,
        Some("long") => Phase::Long,
        _ => Phase::Idle,
    };
    let running = get(s, "pomodoro.running").and_then(Value::as_bool).unwrap_or(false);
    let done = get(s, "pomodoro.done").and_then(Value::as_u64).unwrap_or(0) as u32;
    let rem = Duration::from_millis(get(s, "pomodoro.remaining_ms").and_then(Value::as_u64).unwrap_or(0));
    Pomodoro::from_parts(phase, done, running, rem)
}

/// Pomodoro phase ends so far, and the last one ("break", "long_break", "focus").
pub fn pomodoro_events(s: &Signals) -> (u64, Option<String>) {
    (get(s, "pomodoro.events").and_then(Value::as_u64).unwrap_or(0), text(s, "pomodoro.last_event"))
}

pub fn now_playing(s: &Signals) -> Option<NowPlaying> {
    Some(NowPlaying {
        player: text(s, "media.player")?,
        playing: get(s, "media.playing").and_then(Value::as_bool).unwrap_or(false),
        title: text(s, "media.title").unwrap_or_default(),
        artist: text(s, "media.artist").unwrap_or_default(),
        album: text(s, "media.album").unwrap_or_default(),
        position: num(s, "media.position").unwrap_or(0.0),
        duration: num(s, "media.duration").unwrap_or(0.0),
        art_url: text(s, "media.art_url").unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readers_get_back_what_was_published() {
        let mut c = Core { muted: Some(true), ..Default::default() };
        c.np = Some(NowPlaying {
            player: "Spotify".into(),
            playing: true,
            title: "Song".into(),
            artist: "A".into(),
            album: "B".into(),
            position: 30.5,
            duration: 200.0,
            art_url: "http://x".into(),
        });
        c.weather = Some(Weather { temp: 21.0, code: 3, is_day: true, max: 24.0, min: 12.0, place: "Kiel".into() });
        c.location = Some(Location { name: "Kiel".into(), lat: 54.3, lon: 10.1 });
        c.pomo.press(&c.pomo_cfg);
        let mut s = Signals::default();
        c.publish(&mut Host::new(ID, &mut s, "d200".into()));
        assert_eq!(muted(&s), Some(true));
        assert_eq!(now_playing(&s), c.np);
        let (w, err, configured) = weather(&s);
        assert_eq!(w.map(|w| (w.temp, w.code, w.place)), Some((21.0, 3, "Kiel".to_string())));
        assert!(err.is_none() && configured);
        let p = pomodoro(&s);
        assert_eq!((p.phase, p.running), (Phase::Work, true));
        assert!(p.remaining(&c.pomo_cfg) > Duration::from_secs(24 * 60));
    }

    #[test]
    fn pomodoro_actions() {
        let mut c = Core::default();
        let mut s = Signals::default();
        let mut h = Host::new(ID, &mut s, "d200".into());
        let start = Map::new();
        c.on_action("pomodoro", &start, &mut h).unwrap().unwrap();
        assert!(c.pomo.running);
        let reset = json!({"do": "reset"}).as_object().cloned().unwrap();
        c.on_action("pomodoro", &reset, &mut h).unwrap().unwrap();
        assert_eq!(c.pomo.phase, Phase::Idle);
        assert_eq!(c.on_action("nope", &Map::new(), &mut h), None);
    }
}
