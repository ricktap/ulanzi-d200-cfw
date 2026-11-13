//! com.deckling.bambu: Bambu Lab printers from Home Assistant.
//!
//! The Home Assistant integration (custom_components/d200_deck/bambu.py) finds the printers
//! of the Bambu Lab integration and publishes one retained message per printer on
//! `<base>/bambu/<device id>/state`:
//!   {"name": "P1S", "model": "p1s", "status": "printing", "progress": 34,
//!    "remainingMinutes": 96, "nozzle": 245, "nozzleTarget": 245, "bed": 70, "bedTarget": 70,
//!    "lightOn": true, "filamentColor": "#74B9FF", "layer": 102, "totalLayers": 300,
//!    "spools": [{"material": "PLA", "remaining": 80, "color": "#FFFFFF"}]}
//! An empty payload removes the printer. Over the Web API the same goes to
//! `PUT /api/topics/bambu/<device id>/state`.
//!
//! The app `com.deckling.bambu:printer` shows one printer on a key: the machine (A1, A1 mini
//! or P1S) with its print growing, the state, the percentage and the time left. A press
//! switches to the temperatures and the filament; a long press asks Home Assistant to toggle
//! the chamber light (`<base>/bambu/<device id>/action`, payload `light`).
//!
//! The drawings come from the Night Signal printer apps (machines/*.svgpart); this file is
//! the Rust port of their render function, with the colours taken from the theme.

use crate::bind::{number, Signals};
use crate::config::Key;
use crate::plugins::{Host, Plugin};
use crate::render::{self, Canvas, Fonts, Rgb, Weight};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const ID: &str = "com.deckling.bambu";

const MODELS: [(&str, &str, &str); 3] = [
    ("a1", "A1", include_str!("machines/a1.svgpart")),
    ("a1-mini", "A1 mini", include_str!("machines/a1-mini.svgpart")),
    ("p1s", "P1S", include_str!("machines/p1s.svgpart")),
];

/// "A1MINI", "A1 Mini", "a1-mini" → "a1-mini"; enclosed printers (P1P, X1C ...) draw as the P1S.
pub fn model_of(name: &str) -> &'static str {
    let n: String = name.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_ascii_lowercase();
    match n.as_str() {
        "a1mini" => "a1-mini",
        "a1" => "a1",
        _ => "p1s",
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[derive(Default)]
pub struct Bambu {
    /// device id -> name, for the device picker.
    printers: BTreeMap<String, String>,
}

impl Plugin for Bambu {
    fn id(&self) -> &'static str {
        ID
    }

    fn mqtt_topics(&self) -> &'static [&'static str] {
        &["bambu/+/state"]
    }

    fn on_mqtt(&mut self, topic: &str, payload: &str, host: &mut Host) {
        let ["bambu", id, "state"] = topic.split('/').collect::<Vec<_>>()[..] else { return };
        if !valid_id(id) {
            return;
        }
        host.clear_prefix(&format!("{id}."));
        let v: Value = serde_json::from_str(payload).unwrap_or(Value::Null);
        if !v.is_object() {
            self.printers.remove(id);
            return;
        }
        let name = v["name"].as_str().unwrap_or(id).to_string();
        host.publish(&format!("{id}.name"), json!(name));
        host.publish(&format!("{id}.status"), json!(status_of(&v)));
        if let Some(p) = v.get("progress").and_then(number) {
            host.publish(&format!("{id}.progress"), json!(p.clamp(0.0, 100.0)));
        }
        if let Some(on) = v["lightOn"].as_bool() {
            host.publish(&format!("{id}.light"), json!(on));
        }
        host.publish(&format!("{id}.state"), v);
        self.printers.insert(id.to_string(), name);
    }

    fn on_action(&mut self, name: &str, params: &Map<String, Value>, host: &mut Host) -> Option<Result<(), String>> {
        if name != "light" {
            return None;
        }
        let id = params.get("device").and_then(Value::as_str).unwrap_or("");
        if !valid_id(id) {
            return Some(Err("Bambu Lab: pick the printer in the key's settings".into()));
        }
        host.mqtt(&format!("bambu/{id}/action"), "light", false);
        Some(Ok(()))
    }

    fn options(&self, name: &str) -> Option<Vec<Value>> {
        (name == "devices").then(|| self.printers.iter().map(|(id, name)| json!({ "label": name, "value": id })).collect())
    }

    fn status(&self) -> Value {
        json!({ "printers": self.printers.values().collect::<Vec<_>>() })
    }
}

// ------------------------------------------------------------------ the face

const STATES: [&str; 8] = ["idle", "heating", "calibrating", "printing", "paused", "complete", "error", "offline"];

/// The state a message reports; anything unknown is offline.
fn status_of(v: &Value) -> &'static str {
    let s = v["status"].as_str().unwrap_or("");
    STATES.iter().copied().find(|x| *x == s).unwrap_or("offline")
}

fn percent(v: Option<&Value>) -> Option<f64> {
    v.and_then(number).map(|p| p.clamp(0.0, 100.0))
}

fn temp(v: Option<f64>) -> String {
    v.map_or("—".into(), |t| format!("{:.0}", t.round()))
}

fn eta(v: Option<&Value>) -> String {
    let Some(n) = v.and_then(number) else { return "ETA —".into() };
    let m = n.max(0.0).ceil() as u64;
    if m >= 60 {
        format!("{}h {:02}m", m / 60, m % 60)
    } else {
        format!("{m}m left")
    }
}

/// Where the bed of each model is drawn: (x, y, width).
fn bed(model: &str) -> (f64, f64, f64) {
    match model {
        "a1" => (54.0, 84.0, 51.0),
        "a1-mini" => (70.0, 83.0, 52.0),
        _ => (50.0, 82.0, 61.0),
    }
}

/// The printed block and the moving parts for `progress` (0..100) and `phase` (0..1, the
/// nozzle's sweep across the block).
#[derive(Debug, PartialEq)]
pub struct Pose {
    pub height: f64,
    pub block_x: f64,
    pub block_bottom: f64,
    pub bed_z: f64,
    pub gantry_z: f64,
    pub head_x: f64,
}

pub fn pose(model: &str, progress: Option<f64>, phase: f64) -> Pose {
    let (bx, by, bw) = bed(model);
    let height = 18.0 * progress.unwrap_or(0.0) / 100.0;
    let center = bx + bw / 2.0;
    let tip0 = match model {
        "a1-mini" => 99.0,
        "p1s" => 80.5,
        _ => 79.5,
    };
    let offset = center - tip0;
    let (from, to) = if model == "p1s" { (offset - 9.0, offset + 9.0) } else { (offset + 9.0, offset - 9.0) };
    let bed_z = if model == "p1s" { height - 17.0 } else { 0.0 };
    let gantry_z = if model == "p1s" { 0.0 } else { by - 1.0 - height - 64.0 };
    Pose { height, block_x: center - 9.0, block_bottom: by + bed_z - 1.0, bed_z, gantry_z, head_x: from + (to - from) * phase.clamp(0.0, 1.0) }
}

/// 0 → 1 → 0 over `period` seconds.
fn triangle(t: f64, period: f64) -> f64 {
    let x = (t / (period / 2.0)) % 2.0;
    if x > 1.0 { 2.0 - x } else { x }
}

fn hex(c: Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// "#RRGGBB" from "#RGB", "#RRGGBB" or "#RRGGBBAA".
fn colour(v: &Value) -> Option<String> {
    let s = v.as_str()?.trim();
    let h = s.strip_prefix('#')?;
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match h.len() {
        3 => Some(format!("#{}", h.chars().flat_map(|c| [c, c]).collect::<String>())),
        6 => Some(format!("#{h}")),
        8 => Some(format!("#{}", &h[..6])),
        _ => None,
    }
}

/// The state's colour, from the theme.
fn accent(status: &str) -> Rgb {
    match status {
        "complete" => render::mint(),
        "error" => render::coral(),
        "paused" => render::lavender(),
        "offline" => [0x68, 0x72, 0x7D],
        _ => render::amber(),
    }
}

const LAMP: &str = "#FFF0C8";
const LAMP_OFF: &str = "#535E66";

/// The machine as an SVG (160 x 160, transparent), every colour resolved.
pub fn machine_svg(model: &str, status: &str, progress: Option<f64>, light: Option<bool>, filament: Option<&str>, t: f64) -> String {
    let geometry = MODELS.iter().find(|(m, ..)| *m == model).map_or(MODELS[2].2, |m| m.2);
    let (ink, bg, dim, acc) = (hex(render::ink()), hex(render::bg()), hex(render::dim()), hex(accent(status)));
    let active = matches!(status, "printing" | "paused" | "complete") && progress.is_some();
    let phase = if status == "printing" { triangle(t, 5.0) } else { 0.5 };
    let p = pose(model, progress, phase);
    let cal = triangle(t, 6.4);
    let gantry_z = match status {
        "complete" if model != "p1s" => -11.0,
        "calibrating" if model != "p1s" => -8.0 + 22.0 * cal,
        _ if active => p.gantry_z,
        _ => 0.0,
    };
    let bed_z = match status {
        "calibrating" if model == "p1s" => -16.0 + 22.0 * cal,
        _ if active => p.bed_z,
        _ => 0.0,
    };
    let head_x = match status {
        "complete" => match model {
            "a1" => 22.0,
            "a1-mini" => 4.0,
            _ => -20.0,
        },
        _ if active => p.head_x,
        _ => 0.0,
    };
    let lamp_on = light == Some(true);
    let lamp = if lamp_on { LAMP } else { LAMP_OFF };
    let nozzle = filament.unwrap_or("#87909E");
    let line = format!(r#"fill="none" stroke="{ink}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round""#);
    let mut m = geometry
        .trim()
        .replace("LAMP_ID-lamp", "lamp")
        .replace(r#"<g class="z-gantry""#, &format!(r#"<g transform="translate(0 {gantry_z:.2})""#))
        .replace(r#"<g class="z-bed""#, &format!(r#"<g transform="translate(0 {bed_z:.2})""#))
        .replace(r#"<g class="head""#, &format!(r#"<g transform="translate({head_x:.2} 0)""#))
        .replace(r#"class="line head-body""#, &format!(r#"fill="{bg}" stroke="{ink}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round""#))
        .replace(r#"class="line""#, &line)
        .replace(r#"class="lamp-beam""#, &format!(r#"opacity="{}""#, if lamp_on { "0.23" } else { "0" }))
        .replace(r#"class="lamp-emitter""#, &format!(r#"fill="{lamp}" stroke="none""#))
        .replace(r#"class="lamp-strip""#, &format!(r#"fill="none" stroke="{lamp}" stroke-width="1.6" stroke-linecap="round""#))
        .replace(r#"class="nozzle""#, &format!(r#"fill="{nozzle}" stroke="{nozzle}" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round""#))
        .replace(r#"class="active""#, &format!(r#"fill="none" stroke="{acc}" stroke-width="1.8" stroke-linecap="round""#))
        .replace(r#"class="wire""#, &format!(r#"fill="none" stroke="{dim}" stroke-width="1.2" stroke-linecap="round""#));
    // the lamp's beam ends on the bed, wherever the gantry is
    if model == "a1" {
        m = m.replace("M44 49 51 82 101 82Z", &format!("M44 49 51 {0:.2} 101 {0:.2}Z", 82.0 - gantry_z));
    }
    if model == "a1-mini" {
        m = m.replace("M63 50 75 81 120 81Z", &format!("M63 50 75 {0:.2} 120 {0:.2}Z", 81.0 - gantry_z));
    }
    let body = match progress {
        Some(pr) if pr > 0.0 && active => {
            format!(r#"<rect x="{:.2}" y="{:.2}" width="18" height="{:.2}" rx="1" fill="{acc}" opacity=".78"/>"#, p.block_x, p.block_bottom - p.height, p.height)
        }
        _ => String::new(),
    };
    let (bx, _, bw) = bed(model);
    let centre = bx + bw / 2.0;
    let heat = if status == "heating" {
        [-10.0, 0.0, 10.0]
            .iter()
            .map(|o| format!(r#"<path d="M{} 78c-4-4 4-4 0-8" fill="none" stroke="{acc}" stroke-width="1.6" stroke-linecap="round" opacity=".7"/>"#, centre + o))
            .collect::<String>()
    } else {
        String::new()
    };
    let opacity = match status {
        "offline" => ".35",
        "paused" => ".7",
        _ => "1",
    };
    // the A1 and the A1 mini are drawn mirrored
    let orientation = if model == "p1s" { "translate(0 0)" } else { "translate(160 0) scale(-1 1)" };
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="160" viewBox="0 0 160 160"><defs><linearGradient id="lamp" x1="0" y1="0" x2="{x2}" y2="1"><stop stop-color="{LAMP}" stop-opacity=".9"/><stop offset="1" stop-color="{LAMP}" stop-opacity="0"/></linearGradient></defs><g opacity="{opacity}" transform="translate(11.2 6.4) scale(.86)"><g transform="{orientation}">{body}{m}{heat}</g></g></svg>"#,
        x2 = if model == "p1s" { "0" } else { ".6" },
    )
}

/// The printer the key shows: `config.device`, as Home Assistant last described it.
pub fn printer<'a>(signals: &'a Signals, key: &Key) -> Option<&'a Value> {
    signals.get(&format!("{ID}:{}.state", key.cfg_str("device")?))
}

const K: f32 = 196.0;
/// The drawings use a 160 x 160 canvas.
const S: f32 = K / 160.0;
const SPOOLS: [Rgb; 4] = [[0x69, 0xE0, 0xC1], [0xB7, 0x9A, 0xF7], [0xFF, 0xB4, 0x58], [0xFF, 0x82, 0x78]];

/// The key face. `view`: 0 progress, 1 temperatures, 2 filament (a press cycles). `t`: seconds,
/// for the nozzle's sweep and the calibration moves.
pub fn face(f: &Fonts, key: &Key, data: Option<&Value>, view: usize, t: f64, bg: Rgb) -> Canvas {
    let mut c = render::key_canvas(K as u32, K as u32, bg);
    let (ink, dim) = (render::ink(), render::dim());
    let none = Value::Null;
    let d = data.unwrap_or(&none);
    let model = match key.cfg_str("model").filter(|m| *m != "auto") {
        Some(m) => model_of(m),
        None => model_of(d["model"].as_str().unwrap_or("p1s")),
    };
    let status = if data.is_some() { status_of(d) } else { "offline" };
    let acc = accent(status);
    let right = |c: &mut Canvas, text: &str, size: f32, x: f32, y: f32, col: Rgb| {
        let w = c.tw(f, text, size * S, Weight::Regular, 0.0);
        c.tl(f, text, size * S, x * S - w, y * S, col, Weight::Regular, 0.0);
    };
    let label = |c: &mut Canvas, text: &str, x: f32, y: f32, col: Rgb| {
        c.caps_at(f, text, 8.0 * S, x * S, y * S, col);
    };
    // header: the printer's name and the state dot
    let name = d["name"].as_str().map(str::to_string).unwrap_or_else(|| MODELS.iter().find(|m| m.0 == model).map_or("Printer", |m| m.1).to_string());
    let name = c.fit(f, &name.to_uppercase(), 8.0 * S, 118.0 * S);
    label(&mut c, &name, 13.0, 19.0, dim);
    c.rect(142.5 * S, 13.5 * S, 5.0 * S, 5.0 * S, 2.5 * S, acc, 1.0);

    let known = percent(d.get("progress"));
    let p = if status == "complete" { Some(100.0) } else { known };
    let off = status == "offline";
    let num = |k: &str| if off { None } else { d.get(k).and_then(number) };
    let (n, nt, b, bt) = (num("nozzle"), num("nozzleTarget"), num("bed"), num("bedTarget"));
    let heat = match (n, nt) {
        (Some(n), Some(nt)) if nt > 0.0 => Some((n / nt * 100.0).clamp(0.0, 100.0)),
        _ => None,
    };
    let effective = match status {
        "heating" => heat,
        "idle" | "offline" | "calibrating" => None,
        _ => p,
    };
    let state_label = match status {
        "idle" => "Ready",
        "heating" => "Heating",
        "calibrating" => "Calibrating",
        "printing" => "Printing",
        "paused" => "Paused",
        "complete" => "Print complete",
        "error" => "Needs attention",
        _ if data.is_none() => "No data yet",
        _ => "Unavailable",
    };
    match view % 3 {
        1 => {
            label(&mut c, "Nozzle", 13.0, 34.0, acc);
            c.tl(f, &format!("{}°", temp(n)), 27.0 * S, 12.0 * S, 65.0 * S, ink, Weight::Semi, -1.0);
            right(&mut c, &format!("/ {}°", temp(nt)), 10.0, 147.0, 63.0, dim);
            label(&mut c, "Heated bed", 13.0, 87.0, acc);
            c.tl(f, &format!("{}°", temp(b)), 27.0 * S, 12.0 * S, 119.0 * S, ink, Weight::Semi, -1.0);
            right(&mut c, &format!("/ {}°", temp(bt)), 10.0, 147.0, 117.0, dim);
            label(&mut c, state_label, 13.0, 141.0, dim);
        }
        2 => {
            label(&mut c, "Filament", 13.0, 34.0, acc);
            let spools: Vec<&Value> = d["spools"].as_array().map(|a| a.iter().take(4).collect()).unwrap_or_default();
            if spools.is_empty() {
                c.tl(f, "No feeder data", 10.0 * S, 13.0 * S, 81.0 * S, dim, Weight::Regular, 0.0);
            }
            for (i, s) in spools.iter().enumerate() {
                let y = 52.0 + i as f32 * 25.0;
                let col = colour(&s["color"]).and_then(|h| render::parse_color(&h)).unwrap_or(SPOOLS[i]);
                // a spool: outline, colour, hub
                c.rect(15.5 * S, (y - 8.5) * S, 17.0 * S, 17.0 * S, 8.5 * S, ink, 1.0);
                c.rect(16.5 * S, (y - 7.5) * S, 15.0 * S, 15.0 * S, 7.5 * S, col, 1.0);
                c.rect(22.0 * S, (y - 2.0) * S, 4.0 * S, 4.0 * S, 2.0 * S, bg, 1.0);
                let material = s["material"].as_str().filter(|m| !m.is_empty()).unwrap_or("Filament");
                let material = c.fit(f, material, 9.0 * S, 78.0 * S);
                c.tl(f, &material, 9.0 * S, 40.0 * S, (y + 3.0) * S, ink, Weight::Regular, 0.0);
                let level = percent(s.get("remaining")).map_or("—".to_string(), |l| format!("{:.0}%", l.round()));
                right(&mut c, &level, 10.0, 147.0, y + 3.0, dim);
            }
        }
        _ => {
            let light = if off { None } else { d["lightOn"].as_bool() };
            let filament = colour(&d["filamentColor"]);
            let svg = machine_svg(model, status, p, light, filament.as_deref(), t);
            if let Some(pm) = crate::icons::rasterise_svg(&svg, K as u32) {
                c.draw_pixmap(&pm, 0.0, 0.0, K, K);
            }
            let metric = match status {
                "heating" => "Warmup".to_string(),
                "idle" => "Ready".to_string(),
                "calibrating" => "Setup".to_string(),
                "complete" => "Done".to_string(),
                "error" => "Check".to_string(),
                "offline" => "Offline".to_string(),
                _ => p.map_or("—".to_string(), |p| format!("{:.0}%", p.round())),
            };
            let side = match status {
                "printing" | "paused" => eta(d.get("remainingMinutes")),
                "complete" => "Collect print".to_string(),
                "error" => d["error"].as_str().filter(|e| !e.is_empty()).unwrap_or("View details").to_string(),
                _ => String::new(),
            };
            label(&mut c, state_label, 13.0, 114.0, acc);
            c.tl(f, &metric, 27.0 * S, 12.0 * S, 141.0 * S, ink, Weight::Semi, -1.0);
            let metric_w = c.tw(f, &metric, 27.0 * S, Weight::Semi, -1.0);
            let side = c.fit(f, &side, 10.0 * S, (132.0 * S - metric_w).max(0.0));
            right(&mut c, &side, 10.0, 147.0, 141.0, dim);
        }
    }
    // the thin track along the bottom
    c.rect(12.0 * S, 149.0 * S, 136.0 * S, 2.0 * S, S, render::LINE, 1.0);
    if let Some(e) = effective.filter(|e| *e > 0.0) {
        c.rect(12.0 * S, 149.0 * S, (136.0 * e as f32 / 100.0) * S, 2.0 * S, S, acc, 1.0);
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_and_poses() {
        assert_eq!((model_of("A1MINI"), model_of("A1 Mini"), model_of("A1"), model_of("P1S"), model_of("X1C")), ("a1-mini", "a1-mini", "a1", "p1s", "p1s"));
        // as the original pose(): the block grows to 18 units, the A1 gantry rises with it,
        // the P1S lowers its bed, and the nozzle sweeps the block's 18 units
        let a = pose("a1", Some(100.0), 0.0);
        assert_eq!((a.height, a.block_x, a.block_bottom, a.gantry_z, a.bed_z), (18.0, 70.5, 83.0, 1.0, 0.0));
        let p = pose("p1s", Some(50.0), 1.0);
        assert_eq!((p.height, p.bed_z, p.gantry_z, p.block_bottom, p.head_x), (9.0, -8.0, 0.0, 73.0, 9.0));
        assert_eq!(pose("a1-mini", None, 0.5).height, 0.0);
        assert_eq!(pose("a1", Some(50.0), 0.0).head_x - pose("a1", Some(50.0), 1.0).head_x, 18.0);
    }

    #[test]
    fn machine_svgs_have_no_css_left() {
        for (model, ..) in MODELS {
            for status in STATES {
                let svg = machine_svg(model, status, Some(40.0), Some(true), Some("#74B9FF"), 1.0);
                assert!(!svg.contains("class=") && !svg.contains("var(") && !svg.contains("LAMP_ID"), "{model} {status}: {svg}");
                let pm = crate::icons::rasterise_svg(&svg, 196).unwrap_or_else(|| panic!("{model} {status} does not render"));
                assert!(pm.data().chunks_exact(4).any(|p| p[3] > 0), "{model} {status} is empty");
            }
        }
        // the print block only while printing, in the state's colour; the nozzle in the filament's
        let printing = machine_svg("p1s", "printing", Some(40.0), Some(true), Some("#74B9FF"), 0.0);
        assert!(printing.contains(r##"height="7.20" rx="1" fill="#FFBF69""##) && printing.contains(r##"fill="#74B9FF""##), "{printing}");
        assert!(!machine_svg("p1s", "idle", Some(40.0), None, None, 0.0).contains(r##"rx="1" fill="#FFBF69""##));
        // light off: no beam
        assert!(machine_svg("a1", "idle", None, Some(false), None, 0.0).contains(r#"opacity="0""#));
    }

    #[test]
    fn messages_become_signals_and_options() {
        let mut s = Signals::default();
        let mut b = Bambu::default();
        let mut h = Host::new(ID, &mut s, "d200".into());
        b.on_mqtt("bambu/abc123/state", r##"{"name": "P1S", "model": "p1s", "status": "printing", "progress": 34, "lightOn": true}"##, &mut h);
        b.on_mqtt("bambu/bad id/state", "{}", &mut h);
        assert_eq!(h.get("abc123.status"), Some(&json!("printing")));
        assert_eq!(h.get("abc123.progress"), Some(&json!(34.0)));
        assert_eq!(h.get("abc123.state").unwrap()["model"], "p1s");
        assert_eq!(b.options("devices"), Some(vec![json!({"label": "P1S", "value": "abc123"})]));
        // long press: the light, back to Home Assistant
        let params = json!({"device": "abc123"}).as_object().cloned().unwrap();
        assert_eq!(b.on_action("light", &params, &mut h), Some(Ok(())));
        assert_eq!(h.mqtt_out, vec![("d200/bambu/abc123/action".to_string(), "light".to_string(), false)]);
        assert!(matches!(b.on_action("light", &Map::new(), &mut h), Some(Err(_))));
        // an empty payload removes the printer
        b.on_mqtt("bambu/abc123/state", "", &mut h);
        assert_eq!((h.get("abc123.state"), b.options("devices")), (None, Some(vec![])));
    }

    #[test]
    fn faces_draw_every_view() {
        let Ok(f) = Fonts::load() else { return };
        let data = json!({"name": "P1S", "model": "P1S", "status": "printing", "progress": 34, "remainingMinutes": 96, "nozzle": 245, "nozzleTarget": 245,
                          "bed": 70, "bedTarget": 70, "lightOn": true, "filamentColor": "#74B9FFFF", "spools": [{"material": "PLA", "remaining": 80, "color": "#0000FFFF"}]});
        let key = Key::default();
        let count = |c: &Canvas, rgb: [u8; 3]| c.pm.data().chunks_exact(4).filter(|p| p[0] == rgb[0] && p[1] == rgb[1] && p[2] == rgb[2]).count();
        let views: Vec<Canvas> = (0..3).map(|v| face(&f, &key, Some(&data), v, 0.0, render::BG)).collect();
        assert!(views[0].pm.data() != views[1].pm.data() && views[1].pm.data() != views[2].pm.data());
        assert!(count(&views[0], [0x74, 0xB9, 0xFF]) > 5, "nozzle in the filament's colour");
        assert!(count(&views[2], [0, 0, 255]) > 100, "the spool in its colour");
        // BAMBU_PNG_DIR=<dir>: the faces as PNGs, to look at
        if let Ok(dir) = std::env::var("BAMBU_PNG_DIR") {
            let shots = [
                ("a1-heating", json!({"name": "A1", "model": "A1", "status": "heating", "nozzle": 185, "nozzleTarget": 220, "bed": 55, "bedTarget": 60, "lightOn": true, "filamentColor": "#B79AF7"}), 0),
                ("a1mini-printing", json!({"name": "A1 Mini", "model": "A1MINI", "status": "printing", "progress": 62, "remainingMinutes": 18, "lightOn": true, "filamentColor": "#69E0C1"}), 0),
                ("p1s-printing", data.clone(), 0),
                ("p1s-temps", data.clone(), 1),
                ("p1s-filament", json!({"name": "P1S", "model": "P1S", "status": "printing", "progress": 34, "spools": [{"material": "PLA Matte", "remaining": 0, "color": "#FFFFFFFF"}, {"material": "PLA Matte", "remaining": 11, "color": "#9B9EA0FF"}, {"material": "PETG", "remaining": 65, "color": "#FFB458"}, {"material": "PLA", "color": "#FF8278"}]}), 2),
                ("p1s-complete", json!({"name": "P1S", "model": "P1S", "status": "complete", "progress": 100, "lightOn": false}), 0),
                ("a1-offline", json!({"name": "A1", "model": "A1", "status": "offline"}), 0),
                ("a1mini-idle", json!({"name": "A1 Mini", "model": "A1MINI", "status": "idle", "lightOn": false}), 0),
            ];
            for (name, d, view) in shots {
                face(&f, &key, Some(&d), view, 1.0, render::BG).pm.save_png(format!("{dir}/{name}.png")).unwrap();
            }
        }
        // no data: offline, and it still draws
        let empty = face(&f, &key, None, 0, 0.0, render::BG);
        assert!(empty.pm.data().chunks_exact(4).any(|p| p[0] > 200));
    }
}
