//! Built-in key apps: system monitor, mute, weather, clock, pomodoro.
//! Data collection lives here; the engine owns the instances and calls the faces.

use crate::config::{Key, Location, PomodoroCfg};
use crate::render::{hex, Canvas, Fonts, Rgb, Th, AMBER, BG, CORAL, IVORY, LINE, MINT, MUTED};
use serde_json::Value;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const HIST: usize = 30;

// ------------------------------------------------------------------ system monitor

pub const SYSMON_MODES: [&str; 4] = ["cpu", "ram", "disk", "net"];

pub struct SysStats {
    sys: sysinfo::System,
    disks: sysinfo::Disks,
    nets: sysinfo::Networks,
    comps: sysinfo::Components,
    last_net: Instant,
    last_temp: Option<Instant>,
    pub temp: Option<f32>,
    pub disk_name: String,
    last_disk: Option<Instant>,
    pub cpu: f32,
    pub ram_used: u64,
    pub ram_total: u64,
    pub disk_used: u64,
    pub disk_total: u64,
    pub rx: f64,
    pub tx: f64,
    pub h_cpu: VecDeque<f32>,
    pub h_ram: VecDeque<f32>,
    pub h_rx: VecDeque<f32>,
    pub h_tx: VecDeque<f32>,
}

fn push(h: &mut VecDeque<f32>, v: f32) {
    h.push_back(v);
    while h.len() > HIST {
        h.pop_front();
    }
}

impl SysStats {
    pub fn new() -> Self {
        let mut s = SysStats {
            sys: sysinfo::System::new(),
            disks: sysinfo::Disks::new_with_refreshed_list(),
            nets: sysinfo::Networks::new_with_refreshed_list(),
            comps: sysinfo::Components::new_with_refreshed_list(),
            last_net: Instant::now(),
            last_temp: None,
            temp: None,
            disk_name: String::new(),
            last_disk: None,
            cpu: 0.0,
            ram_used: 0,
            ram_total: 0,
            disk_used: 0,
            disk_total: 0,
            rx: 0.0,
            tx: 0.0,
            h_cpu: VecDeque::new(),
            h_ram: VecDeque::new(),
            h_rx: VecDeque::new(),
            h_tx: VecDeque::new(),
        };
        s.sys.refresh_cpu_usage();
        s.refresh();
        s
    }

    /// Call about once a second.
    pub fn refresh(&mut self) {
        self.sys.refresh_cpu_usage();
        self.cpu = self.sys.global_cpu_usage();
        self.sys.refresh_memory();
        self.ram_used = self.sys.used_memory();
        self.ram_total = self.sys.total_memory();

        self.nets.refresh(true);
        let dt = self.last_net.elapsed().as_secs_f64().max(0.2);
        self.last_net = Instant::now();
        let (mut rx, mut tx) = (0u64, 0u64);
        for (name, n) in self.nets.iter() {
            if name.starts_with("lo") || name.starts_with("utun") || name.starts_with("awdl") || name.starts_with("llw") {
                continue;
            }
            rx += n.received();
            tx += n.transmitted();
        }
        self.rx = rx as f64 / dt;
        self.tx = tx as f64 / dt;

        if self.last_temp.is_none_or(|t| t.elapsed() > Duration::from_secs(5)) {
            self.comps.refresh(false);
            self.last_temp = Some(Instant::now());
            self.temp = cpu_temp(&self.comps);
        }
        if self.last_disk.is_none_or(|t| t.elapsed() > Duration::from_secs(30)) {
            self.disks.refresh(true);
            self.last_disk = Some(Instant::now());
            let main = self
                .disks
                .iter()
                .find(|d| d.mount_point() == std::path::Path::new("/"))
                .or_else(|| self.disks.iter().find(|d| d.mount_point().to_string_lossy().starts_with("C:")))
                .or_else(|| self.disks.iter().next());
            if let Some(d) = main {
                self.disk_name = d.name().to_string_lossy().to_string();
                self.disk_total = d.total_space();
                self.disk_used = d.total_space().saturating_sub(d.available_space());
            }
        }

        push(&mut self.h_cpu, self.cpu);
        let ram_pct = if self.ram_total > 0 { self.ram_used as f32 * 100.0 / self.ram_total as f32 } else { 0.0 };
        push(&mut self.h_ram, ram_pct);
        push(&mut self.h_rx, self.rx as f32);
        push(&mut self.h_tx, self.tx as f32);
    }
}

/// CPU temperature from the host's sensors, if it reports any.
pub fn cpu_temp(c: &sysinfo::Components) -> Option<f32> {
    let temps: Vec<f32> = c
        .iter()
        .filter(|x| {
            let l = x.label().to_ascii_lowercase();
            l.contains("cpu") || l.contains("tdie") || l.contains("tctl") || l.contains("package") || l.contains("core")
        })
        .filter_map(|x| x.temperature())
        .filter(|t| t.is_finite() && *t > 5.0 && *t < 130.0)
        .collect();
    (!temps.is_empty()).then(|| temps.iter().copied().fold(f32::MIN, f32::max))
}

pub fn fmt_bytes(b: f64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b;
    let mut i = 0;
    while v >= 1000.0 && i < units.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if v >= 100.0 || i == 0 {
        format!("{v:.0} {}", units[i])
    } else {
        format!("{v:.1} {}", units[i])
    }
}

pub fn sysmon_modes(key: &Key) -> Vec<&'static str> {
    let modes = key.cfg_list("modes");
    let m: Vec<&'static str> = SYSMON_MODES.iter().copied().filter(|m| modes.is_empty() || modes.iter().any(|x| x == m)).collect();
    if m.is_empty() {
        SYSMON_MODES.to_vec()
    } else {
        m
    }
}

// ------------------------------------------------------------------ mute

// ------------------------------------------------------------------ weather

#[derive(Debug, Clone)]
pub struct Weather {
    pub temp: f64,
    pub code: i64,
    pub is_day: bool,
    pub max: f64,
    pub min: f64,
    pub place: String,
}

fn get_json(url: &str) -> Result<Value, String> {
    let resp = ureq::get(url)
        .timeout(Duration::from_secs(10))
        .set("User-Agent", "d200-deck")
        .call()
        .map_err(|e| e.to_string())?;
    resp.into_json::<Value>().map_err(|e| e.to_string())
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Current weather from Open-Meteo (no API key).
pub fn fetch_weather(loc: &Location, fahrenheit: bool) -> Result<Weather, String> {
    let unit = if fahrenheit { "&temperature_unit=fahrenheit" } else { "" };
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={:.4}&longitude={:.4}&current=temperature_2m,weather_code,is_day&daily=temperature_2m_max,temperature_2m_min&forecast_days=1&timezone=auto{unit}",
        loc.lat, loc.lon
    );
    let v = get_json(&url)?;
    let cur = &v["current"];
    let num = |x: &Value| x.as_f64().ok_or_else(|| format!("unexpected weather data: {v}"));
    Ok(Weather {
        temp: num(&cur["temperature_2m"])?,
        code: cur["weather_code"].as_i64().unwrap_or(0),
        is_day: cur["is_day"].as_i64().unwrap_or(1) == 1,
        max: num(&v["daily"]["temperature_2m_max"][0])?,
        min: num(&v["daily"]["temperature_2m_min"][0])?,
        place: loc.name.clone(),
    })
}

/// Place search for the settings dialog.
pub fn geocode(query: &str) -> Result<Vec<Value>, String> {
    let url = format!("https://geocoding-api.open-meteo.com/v1/search?count=6&language=en&name={}", url_encode(query.trim()));
    let v = get_json(&url)?;
    Ok(v["results"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|r| {
            let name = r["name"].as_str().unwrap_or("").to_string();
            let detail = [r["admin1"].as_str(), r["country"].as_str()].into_iter().flatten().collect::<Vec<_>>().join(", ");
            serde_json::json!({ "name": name, "detail": detail, "lat": r["latitude"], "lon": r["longitude"] })
        })
        .collect())
}

/// WMO weather code -> (icon, description).
pub fn weather_info(code: i64, day: bool) -> (&'static str, &'static str) {
    match code {
        0 => (if day { "weather-sun" } else { "weather-moon" }, "Clear"),
        1 => (if day { "weather-partly-cloudy" } else { "weather-moon" }, "Mostly clear"),
        2 => ("weather-partly-cloudy", "Partly cloudy"),
        3 => ("weather-cloudy", "Cloudy"),
        45 | 48 => ("weather-cloudy", "Fog"),
        51..=57 => ("weather-rain", "Drizzle"),
        61..=67 | 80..=82 => ("weather-rain", "Rain"),
        71..=77 | 85 | 86 => ("weather-snow", "Snow"),
        95..=99 => ("weather-storm", "Thunderstorm"),
        _ => ("temperature", "Weather"),
    }
}

// ------------------------------------------------------------------ clock

// ------------------------------------------------------------------ pomodoro

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Work,
    Short,
    Long,
}

#[derive(Debug, Clone)]
pub struct Pomodoro {
    pub phase: Phase,
    /// Completed work sessions.
    pub done: u32,
    pub running: bool,
    remaining: Duration,
    deadline: Option<Instant>,
}

pub enum PomoEvent {
    WorkDone { long_break: bool },
    BreakDone,
}

impl Pomodoro {
    pub fn new() -> Self {
        Pomodoro { phase: Phase::Idle, done: 0, running: false, remaining: Duration::ZERO, deadline: None }
    }

    pub fn len(phase: Phase, cfg: &PomodoroCfg) -> Duration {
        let m = match phase {
            Phase::Idle | Phase::Work => cfg.work,
            Phase::Short => cfg.short_break,
            Phase::Long => cfg.long_break,
        };
        Duration::from_secs(m.max(1) as u64 * 60)
    }

    pub fn remaining(&self, cfg: &PomodoroCfg) -> Duration {
        match (self.phase, self.deadline) {
            (Phase::Idle, _) => Self::len(Phase::Work, cfg),
            (_, Some(d)) if self.running => d.saturating_duration_since(Instant::now()),
            _ => self.remaining,
        }
    }

    fn start(&mut self, phase: Phase, cfg: &PomodoroCfg, run: bool) {
        self.phase = phase;
        self.remaining = Self::len(phase, cfg);
        self.running = run;
        self.deadline = run.then(|| Instant::now() + self.remaining);
    }

    /// Short press: start / pause / resume.
    pub fn press(&mut self, cfg: &PomodoroCfg) {
        if self.phase == Phase::Idle {
            self.start(Phase::Work, cfg, true);
        } else if self.running {
            self.remaining = self.remaining(cfg);
            self.running = false;
            self.deadline = None;
        } else {
            self.running = true;
            self.deadline = Some(Instant::now() + self.remaining);
        }
    }

    /// Long press: reset.
    /// A timer as the core reads it back from signals (`core_plugin::pomodoro`).
    pub fn from_parts(phase: Phase, done: u32, running: bool, remaining: Duration) -> Self {
        Pomodoro { phase, done, running, remaining, deadline: running.then(|| Instant::now() + remaining) }
    }

    pub fn reset(&mut self) {
        *self = Pomodoro::new();
    }

    /// Call every second; returns an event when a phase ends.
    pub fn tick(&mut self, cfg: &PomodoroCfg) -> Option<PomoEvent> {
        if !self.running || self.remaining(cfg) > Duration::ZERO {
            return None;
        }
        match self.phase {
            Phase::Work => {
                self.done += 1;
                let long = cfg.rounds > 0 && self.done % cfg.rounds.max(1) == 0;
                self.start(if long { Phase::Long } else { Phase::Short }, cfg, true);
                Some(PomoEvent::WorkDone { long_break: long })
            }
            Phase::Short | Phase::Long => {
                // Wait for a press before the next focus session.
                self.start(Phase::Work, cfg, false);
                Some(PomoEvent::BreakDone)
            }
            Phase::Idle => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pomodoro_cycle() {
        let cfg = PomodoroCfg { work: 1, short_break: 1, long_break: 2, rounds: 2, sound: false };
        let mut p = Pomodoro::new();
        p.press(&cfg);
        assert_eq!(p.phase, Phase::Work);
        assert!(p.running);
        // pause keeps the remaining time
        p.press(&cfg);
        assert!(!p.running);
        let r = p.remaining(&cfg);
        std::thread::sleep(Duration::from_millis(30));
        assert_eq!(p.remaining(&cfg), r);
        // force the end of the phase
        p.press(&cfg);
        p.deadline = Some(Instant::now());
        assert!(matches!(p.tick(&cfg), Some(PomoEvent::WorkDone { long_break: false })));
        assert_eq!(p.phase, Phase::Short);
        p.deadline = Some(Instant::now());
        assert!(matches!(p.tick(&cfg), Some(PomoEvent::BreakDone)));
        assert_eq!(p.phase, Phase::Work);
        assert!(!p.running);
        p.press(&cfg);
        p.deadline = Some(Instant::now());
        assert!(matches!(p.tick(&cfg), Some(PomoEvent::WorkDone { long_break: true })));
        assert_eq!(p.phase, Phase::Long);
        p.reset();
        assert_eq!(p.phase, Phase::Idle);
    }

    /// Network: `cargo test -- --ignored weather_live`
    #[test]
    #[ignore]
    fn weather_live() {
        let r = geocode("Berlin").unwrap();
        assert!(!r.is_empty());
        let loc = Location { name: "Berlin".into(), lat: r[0]["lat"].as_f64().unwrap(), lon: r[0]["lon"].as_f64().unwrap() };
        let w = fetch_weather(&loc, false).unwrap();
        println!("{r:?}\n{w:?}");
        assert!(w.temp > -60.0 && w.temp < 60.0);
    }

    #[test]
    fn bytes() {
        assert_eq!(fmt_bytes(999.0), "999 B");
        assert_eq!(fmt_bytes(1_234_000.0), "1.2 MB");
        assert_eq!(fmt_bytes(512_000_000_000.0), "512 GB");
    }
}

// ------------------------------------------------------------------ media

use crate::media::NowPlaying;
use crate::teams::{presence_style, Meeting, Presence};
use image::DynamicImage;

// ------------------------------------------------------------------ Teams

/// State of the link to the Teams desktop app.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct TeamsLinkState {
    pub connected: bool,
    pub paired: bool,
    pub error: Option<String>,
}

// ------------------------------------------------------------------ debugger



// ------------------------------------------------------------------ faces (Night Signal style)
//
// Keys are near-black with ivory line icons, uppercase letter-spaced labels,
// big bold figures and mint / coral status pills.

const K: f32 = 196.0;
const CX: f32 = 98.0;

fn key(bg: Rgb) -> Canvas {
    Canvas::new(K as u32, K as u32, bg)
}

/// Icon on top, label and optional pill below: the standard key layout.
fn icon_label_pill(f: &Fonts, bg: Rgb, icon: &str, ovr: &[(&str, &str)], label: Option<&str>, pill: Option<(&str, Rgb)>) -> Canvas {
    let mut c = key(bg);
    match (label, pill) {
        (Some(l), Some((p, col))) => {
            c.set_icon(icon, 52.0, 14.0, 92.0, 92.0, ovr);
            c.caps(f, l, 19.0, CX, 134.0, IVORY, 176.0);
            c.pill(f, p, CX, 164.0, col);
        }
        (None, Some((p, col))) => {
            c.set_icon(icon, 46.0, 20.0, 104.0, 104.0, ovr);
            c.pill(f, p, CX, 158.0, col);
        }
        (Some(l), None) => {
            c.set_icon(icon, 46.0, 22.0, 104.0, 104.0, ovr);
            c.caps(f, l, 22.0, CX, 168.0, IVORY, 176.0);
        }
        (None, None) => {
            c.set_icon(icon, 38.0, 38.0, 120.0, 120.0, ovr);
        }
    }
    c
}

// -- system monitor

const PAD: f32 = 12.0;

/// Widget header: caps title left, icon right.
fn widget_head(f: &Fonts, c: &mut Canvas, title: &str, icon: &str, th: &Th) {
    c.caps_at(f, title, 16.0, PAD + 2.0, 32.0, th.text);
    let o = th.icon_overrides();
    let ovr: Vec<(&str, &str)> = o.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    c.set_icon(icon, K - PAD - 28.0, 9.0, 28.0, 28.0, &ovr);
}

/// Thin line graph (no fill) in the bottom band.
fn trace(c: &mut Canvas, vals: &[f32], lo: f32, hi: f32, y: f32, h: f32, col: Rgb) {
    let v: Vec<f32> = vals.iter().map(|v| (v - lo).max(0.0)).collect();
    c.line_graph(PAD, y, K - 2.0 * PAD, h, &v, (hi - lo).max(1e-3), col);
}

/// "2.8" + "MB/s": rates shown consistently in MB/s, KB/s for tiny, GB/s for huge.
pub fn rate(bps: f64) -> (String, &'static str) {
    let mb = bps / 1e6;
    if mb < 0.1 {
        (format!("{:.0}", bps / 1e3), "KB/s")
    } else if mb >= 1000.0 {
        (format!("{:.1}", mb / 1e3), "GB/s")
    } else if mb >= 100.0 {
        (format!("{mb:.0}"), "MB/s")
    } else {
        (format!("{mb:.1}"), "MB/s")
    }
}

fn short_bytes(b: u64) -> String {
    // "680 GB", "1 TB", "34.4 GB"
    let v = b as f64;
    if v >= 1e12 {
        let t = v / 1e12;
        if t >= 10.0 || (t - t.round()).abs() < 0.05 { format!("{:.0} TB", t) } else { format!("{t:.1} TB") }
    } else {
        let g = v / 1e9;
        if g >= 100.0 { format!("{g:.0} GB") } else { format!("{g:.1} GB") }
    }
}

/// Square system widgets (CPU, RAM, disk, network), per the Night Signal widget spec.
pub fn sysmon_face(f: &Fonts, s: &crate::core_plugin::SysView, mode: &str, idx: usize, n: usize, th: &Th) -> Canvas {
    let mut c = key(th.bg);
    th.surface(&mut c, true);
    let (ivory, secondary) = (th.text, th.secondary);
    // which view of `n` (a press cycles)
    if n > 1 {
        for i in 0..n {
            let x = K - PAD - 4.0 - (n - 1 - i) as f32 * 9.0;
            c.rect(x, 188.0, 5.0, 3.0, 1.5, if i == idx { ivory } else { LINE }, 1.0);
        }
    }
    let pct = |a: u64, b: u64| if b > 0 { a as f32 * 100.0 / b as f32 } else { 0.0 };
    let gib = |b: u64| b as f64 / (1u64 << 30) as f64;
    let range = |h: &Vec<f32>| {
        let lo = h.iter().copied().fold(f32::MAX, f32::min);
        let hi = h.iter().copied().fold(f32::MIN, f32::max);
        if lo > hi { (0.0, 100.0) } else { ((lo - 4.0).max(0.0), (hi + 4.0).min(100.0).max(lo + 8.0)) }
    };
    match mode {
        "cpu" | "ram" => {
            let cpu = mode == "cpu";
            widget_head(f, &mut c, if cpu { "CPU" } else { "RAM" }, if cpu { "cpu" } else { "ram" }, th);
            let v = if cpu { s.cpu } else { pct(s.ram_used, s.ram_total) };
            c.figure_left(f, &format!("{v:.0}%"), 64.0, PAD, 104.0, ivory, K - 2.0 * PAD);
            let sub = if cpu {
                s.temp.map(|t| format!("{t:.0}°C")).unwrap_or_default()
            } else {
                format!("{:.1} / {:.1} GB", gib(s.ram_used), gib(s.ram_total))
            };
            c.tl(f, &sub, 18.0, PAD + 2.0, 134.0, secondary, crate::render::Weight::Regular, 0.0);
            let h = if cpu { &s.h_cpu } else { &s.h_ram };
            let (lo, hi) = range(h);
            let vals: Vec<f32> = h.iter().copied().collect();
            trace(&mut c, &vals, lo, hi, 148.0, 32.0, th.chart);
        }
        "disk" => {
            widget_head(f, &mut c, "DISK", "disk", th);
            let v = pct(s.disk_used, s.disk_total);
            c.figure_left(f, &format!("{v:.0}%"), 64.0, PAD, 104.0, ivory, K - 2.0 * PAD);
            let sub = format!("{} / {}", short_bytes(s.disk_used), short_bytes(s.disk_total));
            c.tl(f, &sub, 18.0, PAD + 2.0, 134.0, secondary, crate::render::Weight::Regular, 0.0);
            // ten segments
            let (seg, gap) = (13.8, 3.4);
            let lit = (v / 10.0).round() as usize;
            for i in 0..10 {
                let col = if i < lit { if v > 90.0 { CORAL } else { th.progress } } else { th.track };
                c.rect(PAD + i as f32 * (seg + gap), 156.0, seg, 14.0, 2.5, col, 1.0);
            }
        }
        _ => {
            widget_head(f, &mut c, "NETWORK", "network-traffic", th);
            let row = |c: &mut Canvas, icon: &str, bps: f64, y: f32| {
                let col = if icon == "network-download" { hex(th.down) } else { hex(th.up) };
                c.set_icon(icon, PAD - 4.0, y - 32.0, 32.0, 32.0, &[("icon-mint", col.as_str())]);
                let (v, unit) = rate(bps);
                c.tl(f, &v, 40.0, PAD + 30.0, y, ivory, crate::render::Weight::Bold, -0.6);
                let uw = c.tw(f, unit, 14.0, crate::render::Weight::Regular, 0.0);
                c.tl(f, unit, 14.0, K - PAD - uw, y - 2.0, secondary, crate::render::Weight::Regular, 0.0);
            };
            // ↑ mint (upload), ↓ lavender (download)
            row(&mut c, "network-upload", s.tx, 82.0);
            row(&mut c, "network-download", s.rx, 128.0);
            let max = s.h_rx.iter().chain(s.h_tx.iter()).copied().fold(10_000.0f32, f32::max);
            let tx: Vec<f32> = s.h_tx.iter().copied().collect();
            let rx: Vec<f32> = s.h_rx.iter().copied().collect();
            c.line_graph(PAD, 142.0, K - 2.0 * PAD, 18.0, &tx, max, th.up);
            c.line_graph(PAD, 164.0, K - 2.0 * PAD, 18.0, &rx, max, th.down);
        }
    }
    c
}


// -- mute

pub fn mute_face(f: &Fonts, muted: Option<bool>, bg: Rgb, _fg: Rgb) -> Canvas {
    match muted {
        Some(true) => icon_label_pill(f, bg, "volume-muted", &[], Some("Sound"), Some(("Muted", CORAL))),
        Some(false) => icon_label_pill(f, bg, "volume-on", &[], Some("Sound"), Some(("On", MINT))),
        None => icon_label_pill(f, bg, "volume-on", &[("icon-mint", "#87909E")], Some("Sound"), Some(("N/A", MUTED))),
    }
}

// -- weather

pub fn weather_face(f: &Fonts, w: Option<&Weather>, err: Option<&str>, configured: bool, bg: Rgb, _fg: Rgb) -> Canvas {
    let Some(w) = w else {
        let mut c = key(bg);
        c.set_icon("weather-location", 54.0, 20.0, 88.0, 88.0, &[]);
        let (a, b) = if !configured { ("Set location", "in settings") } else if err.is_some() { ("No data", "retrying") } else { ("Weather", "loading") };
        c.caps(f, a, 17.0, CX, 140.0, IVORY, 176.0);
        c.caps(f, b, 12.0, CX, 164.0, MUTED, 176.0);
        return c;
    };
    let mut c = key(bg);
    let (icon, _) = weather_info(w.code, w.is_day);
    c.set_icon(icon, 58.0, 16.0, 80.0, 80.0, &[]);
    c.figure(f, &format!("{:.0}°", w.temp), 60.0, CX + 6.0, 150.0, IVORY, 176.0);
    c.caps(f, &w.place, 15.0, CX, 180.0, IVORY, 176.0);
    c
}

// -- clock

/// mode 0: time, mode 1: date.
pub fn clock_face(f: &Fonts, mode: usize, h12: bool, th: &Th) -> Canvas {
    use chrono::Timelike;
    let now = chrono::Local::now();
    let mut c = key(th.bg);
    th.surface(&mut c, true);
    let (ivory, secondary, mint, line) = (th.text, th.secondary, th.progress, th.track);
    if mode % 2 == 0 {
        c.caps_at(f, "CLOCK", 16.0, PAD + 2.0, 32.0, ivory);
        // live analog clock: hands follow the time, the ring the seconds
        let props = crate::migrate::legacy_props(
            "analog-clock",
            &serde_json::json!({
                "progress": now.second() as f64 * 100.0 / 60.0,
                "time": {"hours": now.hour(), "minutes": now.minute(), "seconds": now.second()},
            }),
        );
        let theme: serde_json::Map<String, serde_json::Value> = [("accent".to_string(), serde_json::json!(hex(th.accent)))].into_iter().collect();
        let mut env = crate::render::IconEnv::with_props(&props);
        env.ctx.theme = &theme;
        c.set_icon_env("analog-clock", K - PAD - 30.0, 7.0, 32.0, 32.0, &[], Some(&env));
        let t = if h12 { now.format("%-I:%M").to_string() } else { now.format("%H:%M").to_string() };
        c.figure_left(f, &t, 60.0, PAD, 104.0, ivory, K - 2.0 * PAD);
        let sub = if h12 { now.format("%a · %-d %b · %p").to_string() } else { now.format("%a · %-d %b").to_string() };
        c.caps_at(f, &sub, 15.0, PAD + 2.0, 136.0, secondary);
        let sec = now.second() as f32 / 60.0;
        c.rect(PAD, 164.0, K - 2.0 * PAD, 4.0, 2.0, line, 1.0);
        c.rect(PAD, 164.0, ((K - 2.0 * PAD) * sec).max(4.0), 4.0, 2.0, mint, 1.0);
    } else {
        let day = now.format("%a").to_string();
        let dw = c.tw(f, &day.to_uppercase(), 18.0, crate::render::Weight::Semi, 18.0 * 0.14);
        c.caps(f, &day, 18.0, CX, 46.0, ivory, 120.0);
        c.hline(24.0, CX - dw / 2.0 - 12.0, 40.0, line);
        c.hline(CX + dw / 2.0 + 12.0, 172.0, 40.0, line);
        c.figure(f, &now.format("%-d").to_string(), 96.0, CX, 140.0, ivory, 180.0);
        c.caps(f, &now.format("%b %Y").to_string(), 18.0, CX, 176.0, ivory, 176.0);
    }
    c
}

// -- pomodoro

pub fn pomodoro_face(f: &Fonts, p: &Pomodoro, cfg: &PomodoroCfg, bg: Rgb, _fg: Rgb) -> Canvas {
    let mut c = key(bg);
    let rem = p.remaining(cfg);
    let total = Pomodoro::len(if p.phase == Phase::Idle { Phase::Work } else { p.phase }, cfg);
    let secs = rem.as_secs() + u64::from(rem.subsec_millis() > 0);
    let t = format!("{:02}:{:02}", secs / 60, secs % 60);
    let (col, label) = match p.phase {
        Phase::Idle => (MUTED, "Focus"),
        Phase::Work => (AMBER, "Focus"),
        Phase::Short => (MINT, "Break"),
        Phase::Long => (MINT, "Long break"),
    };
    let frac = if p.phase == Phase::Idle { 0.0 } else { 1.0 - rem.as_secs_f32() / total.as_secs_f32() };
    c.ring(CX, CX, 80.0, 9.0, frac, col, [42, 47, 55]);
    c.caps(f, label, 14.0, CX, 74.0, IVORY, 120.0);
    c.figure(f, &t, 48.0, CX, 120.0, IVORY, 136.0);
    let sub = match (p.phase, p.running) {
        (Phase::Idle, _) => "press to start".to_string(),
        (Phase::Work, false) if p.remaining == total => "press to focus".to_string(),
        (_, false) => "paused".to_string(),
        (Phase::Work, true) => format!("round {}/{}", p.done % cfg.rounds.max(1) + 1, cfg.rounds.max(1)),
        _ => String::new(),
    };
    c.caps(f, &sub, 11.0, CX, 146.0, MUTED, 110.0);
    c
}

// -- media

/// Play/pause key: the state ring, an equaliser while playing.
pub fn media_face(f: &Fonts, np: Option<&NowPlaying>, _art: Option<&DynamicImage>, bg: Rgb, _fg: Rgb) -> Canvas {
    let mut c = key(bg);
    let Some(np) = np else {
        c.set_icon("now-playing", 54.0, 20.0, 88.0, 88.0, &[]);
        c.caps(f, "Nothing playing", 15.0, CX, 156.0, MUTED, 176.0);
        return c;
    };
    c.circle(CX, 74.0, 52.0, 2.6, IVORY);
    c.set_icon(if np.playing { "pause" } else { "play" }, CX - 34.0, 40.0, 68.0, 68.0, &[("icon-mint", "#F4F1E8")]);
    // equaliser: 11 bars; animated by track position while playing
    let n = 11;
    let (bw, gap) = (6.0, 5.0);
    let x0 = CX - (n as f32 * (bw + gap) - gap) / 2.0;
    for i in 0..n {
        let hgt = if np.playing {
            let ph = np.position as f32 * 2.3 + i as f32 * 1.7;
            6.0 + 14.0 * (0.5 + 0.5 * ph.sin()) * (0.6 + 0.4 * ((i * 7 % 5) as f32 / 4.0))
        } else {
            4.0
        };
        let col = if np.playing { MINT } else { MUTED };
        c.rect(x0 + i as f32 * (bw + gap), 150.0 - hgt, bw, hgt, 1.5, col, 1.0);
    }
    c.caps(f, if np.playing { "Playing" } else { "Paused" }, 20.0, CX, 182.0, IVORY, 176.0);
    c
}

// -- Teams


const GREY_ICON: [(&str, &str); 3] = [("icon-mint", "#87909E"), ("icon-coral", "#87909E"), ("icon-base", "#87909E")];

/// kind: "mute" or "camera".
pub fn teams_face(f: &Fonts, kind: &str, link: &TeamsLinkState, m: Option<&Meeting>, bg: Rgb) -> Canvas {
    let mic = kind == "mute";
    let (on_icon, off_icon) = if mic { ("microphone-on", "microphone-muted") } else { ("camera-on", "camera-off") };
    if !link.connected {
        let mut c = icon_label_pill(f, bg, on_icon, &GREY_ICON, None, None);
        c.rect(0.0, 150.0, K, 46.0, 0.0, bg, 1.0);
        c.caps(f, "Teams off", 14.0, CX, 176.0, MUTED, 176.0);
        return c;
    }
    let Some(m) = m.filter(|m| m.in_meeting) else {
        let mut c = icon_label_pill(f, bg, on_icon, &GREY_ICON, None, None);
        c.rect(0.0, 150.0, K, 46.0, 0.0, bg, 1.0);
        c.caps(f, "No meeting", 14.0, CX, 176.0, MUTED, 176.0);
        return c;
    };
    let active = if mic { !m.muted } else { m.video_on };
    match (mic, active) {
        (true, true) => icon_label_pill(f, bg, on_icon, &[], None, Some(("Live", MINT))),
        (true, false) => icon_label_pill(f, bg, off_icon, &[], None, Some(("Muted", CORAL))),
        (false, true) => icon_label_pill(f, bg, on_icon, &[], Some("Camera"), Some(("On", MINT))),
        (false, false) => icon_label_pill(f, bg, off_icon, &[], Some("Camera"), Some(("Off", CORAL))),
    }
}

pub fn presence_face(f: &Fonts, p: Option<&Presence>, signed_in: bool, err: Option<&str>, bg: Rgb) -> Canvas {
    if !signed_in {
        let mut c = icon_label_pill(f, bg, "teams", &[], None, None);
        c.rect(0.0, 150.0, K, 46.0, 0.0, bg, 1.0);
        c.caps(f, "Sign in", 15.0, CX, 172.0, MUTED, 176.0);
        return c;
    }
    let Some(p) = p else {
        let mut c = icon_label_pill(f, bg, "teams", &GREY_ICON, None, None);
        c.rect(0.0, 150.0, K, 46.0, 0.0, bg, 1.0);
        c.caps(f, if err.is_some() { "Unavailable" } else { "Loading" }, 14.0, CX, 172.0, MUTED, 176.0);
        return c;
    };
    let (icon, text, col) = match p.availability.as_str() {
        "Available" | "AvailableIdle" => ("presence-available", "Available", MINT),
        "Away" => ("presence-away", "Away", AMBER),
        "BeRightBack" => ("presence-away", "Be right back", AMBER),
        "Busy" | "BusyIdle" => ("do-not-disturb", "Busy", CORAL),
        "DoNotDisturb" => ("do-not-disturb", "Do not disturb", CORAL),
        "Offline" => ("teams", "Offline", MUTED),
        _ => ("teams", "Unknown", MUTED),
    };
    let _ = presence_style;
    let mut c = key(bg);
    c.set_icon(icon, 46.0, 20.0, 104.0, 104.0, &[]);
    // pill text can be long ("Do not disturb"): shrink via caps width check
    let t = text.to_uppercase();
    if c.tw(f, &t, 17.0, crate::render::Weight::Bold, 2.4) + 34.0 > 184.0 {
        c.rect(14.0, 142.0, 168.0, 32.0, 16.0, col, 1.0);
        c.caps(f, text, 13.0, CX, 163.0, BG, 156.0);
    } else {
        c.pill(f, text, CX, 158.0, col);
    }
    c
}

// -- debugger

pub fn debugger_face(f: &Fonts, active: Option<&str>, bg: Rgb, _fg: Rgb) -> Canvas {
    match active {
        Some(_) => icon_label_pill(f, bg, "debug-active", &[], Some("Debug"), Some(("Running", MINT))),
        None => icon_label_pill(f, bg, "debug-active", &[("icon-mint", "#87909E")], Some("Debug"), Some(("Idle", [58, 64, 74]))),
    }
}

#[cfg(test)]
mod temp_probe {
    /// `cargo test -- --ignored temp_dump --nocapture`
    #[test]
    #[ignore]
    fn temp_dump() {
        let c = sysinfo::Components::new_with_refreshed_list();
        for x in c.iter() {
            println!("{:?} = {:?}", x.label(), x.temperature());
        }
        println!("cpu temp: {:?}", super::cpu_temp(&c));
    }
}
