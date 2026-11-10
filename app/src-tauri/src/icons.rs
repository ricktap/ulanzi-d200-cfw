//! The bundled "Night Signal" icon set (215 SVGs, 64 × 64, line style).
//! An icon is referenced by name in the config, e.g. "icon": "volume-on".

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tiny_skia::Pixmap;

include!(concat!(env!("OUT_DIR"), "/icons_gen.rs"));

pub fn svg(name: &str) -> Option<&'static str> {
    ICONS.binary_search_by(|(n, _)| n.cmp(&name)).ok().map(|i| ICONS[i].1)
}

pub fn exists(name: &str) -> bool {
    svg(name.trim()).is_some()
}

/// The set's manifest (names and groups), embedded so the UI does not need to fetch it.
pub static MANIFEST: &str = include_str!("../../ui/icons/night-signal/manifest.json");

pub fn names() -> impl Iterator<Item = &'static str> {
    ICONS.iter().map(|(n, _)| *n)
}

/// Resolve `var(--name, fallback)` (the SVG renderer has no CSS variables),
/// using `overrides` ("icon-mint" -> "#00ff00") where given.
pub fn resolve_vars(src: &str, overrides: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(i) = rest.find("var(--") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 6..];
        let Some(end) = after.find(')') else {
            out.push_str(&rest[i..]);
            return out;
        };
        let inner = &after[..end];
        let (name, fallback) = inner.split_once(',').map(|(a, b)| (a.trim(), b.trim())).unwrap_or((inner.trim(), "#F4F1E8"));
        let v = overrides.iter().find(|(k, _)| *k == name).map(|(_, v)| *v).unwrap_or(fallback);
        out.push_str(v);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The stateful icons and their parts (see STATEFUL.md next to the SVGs).
pub const STATEFUL: [&str; 8] = ["volume-muted", "light", "thermostat", "pomodoro", "analog-clock", "bambu-ams", "bambu-ams-lite", "downloads"];

pub fn is_stateful(name: &str) -> bool {
    STATEFUL.contains(&name.trim())
}

/// Drop CSS the renderer cannot evaluate (calc() and CSS transforms) from inline
/// styles; the presentation attributes beside them carry the same defaults.
fn strip_css(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(i) = rest.find("style=\"") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 7..];
        let Some(end) = after.find('"') else {
            out.push_str(&rest[i..]);
            return out;
        };
        let kept: Vec<&str> = after[..end]
            .split(';')
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .filter(|d| {
                let prop = d.split(':').next().unwrap_or("").trim();
                !(d.contains("calc(") || prop == "transform" || prop == "transform-origin")
            })
            .collect();
        if !kept.is_empty() {
            out.push_str("style=\"");
            out.push_str(&kept.join(";"));
            out.push('"');
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Set attribute `attr` on the element carrying `data-part="part"`.
fn set_part_attr(svg: &mut String, part: &str, attr: &str, value: &str) {
    let marker = format!("data-part=\"{part}\"");
    let Some(m) = svg.find(&marker) else { return };
    let start = svg[..m].rfind('<').unwrap_or(0);
    let end = m + svg[m..].find('>').unwrap_or(0);
    let tag = &svg[start..end];
    let needle = format!(" {attr}=\"");
    let new_tag = if let Some(i) = tag.find(&needle) {
        let vs = i + needle.len();
        let ve = vs + tag[vs..].find('"').unwrap_or(0);
        format!("{}{}{}", &tag[..vs], value, &tag[ve..])
    } else {
        let t = tag.trim_end_matches('/');
        let closing = if tag.ends_with('/') { "/" } else { "" };
        format!("{t} {attr}=\"{value}\"{closing}")
    };
    svg.replace_range(start..end, &new_tag);
}

fn pct(v: &serde_json::Value) -> Option<f64> {
    v.as_f64().map(|x| x.clamp(0.0, 100.0))
}

/// Port of `setIconState` (icon-state.js): the same state object, applied to the SVG text.
/// state: {color, progress, temperature, on, fill, time:{hours,minutes,seconds}, spools:[{color,progress}]}
pub fn apply_state(name: &str, src: &str, state: &serde_json::Value) -> String {
    use serde_json::Value;
    let s = |k: &str| state.get(k).and_then(Value::as_str).map(str::to_string);
    let mut vars: Vec<(String, String)> = vec![];
    let mut dash: Vec<(String, f64)> = vec![]; // (part, percent)
    let mut hands: Vec<(&str, f64)> = vec![];
    let mut temp: Option<f64> = None;
    match name {
        "volume-muted" => {
            if let Some(c) = s("color") {
                vars.push(("mute-color".into(), c));
            }
        }
        "light" => {
            let on = state.get("on").and_then(Value::as_bool).unwrap_or(true);
            vars.push(("light-fill".into(), if on { s("color").unwrap_or_else(|| "#69E0C1".into()) } else { "transparent".into() }));
        }
        "thermostat" | "pomodoro" => {
            if let Some(c) = s("color") {
                vars.push((format!("{name}-color"), c));
            }
            if let Some(p) = state.get("progress").and_then(pct) {
                dash.push((format!("{name}-ring"), p));
            }
            if name == "thermostat" {
                temp = state.get("temperature").and_then(Value::as_f64);
                if let Some(c) = s("number_color").or(s("numberColor")) {
                    vars.push(("thermostat-number-color".into(), c));
                }
            }
        }
        "analog-clock" => {
            if let Some(c) = s("color") {
                vars.push(("clock-color".into(), c));
            }
            if let Some(f) = s("fill") {
                vars.push(("clock-fill".into(), f));
            }
            if let Some(p) = state.get("progress").and_then(pct) {
                dash.push(("clock-ring".into(), p));
            }
            if let Some(t) = state.get("time") {
                let g = |k: &str| t.get(k).and_then(Value::as_f64).unwrap_or(0.0);
                let (h, m, sec) = (g("hours"), g("minutes"), g("seconds"));
                hands.push(("hour", (h % 12.0) * 30.0 + m * 0.5 + sec / 120.0));
                hands.push(("minute", m * 6.0 + sec * 0.1));
                hands.push(("second", sec * 6.0));
            }
        }
        "bambu-ams" | "bambu-ams-lite" => {
            for (i, sp) in state.get("spools").and_then(Value::as_array).into_iter().flatten().take(4).enumerate() {
                if let Some(c) = sp.get("color").and_then(Value::as_str) {
                    vars.push((format!("spool-{}-color", i + 1), c.to_string()));
                }
                if let Some(p) = sp.get("progress").and_then(pct) {
                    dash.push((format!("spool-{}-ring", i + 1), p));
                }
            }
        }
        "downloads" => {
            if let Some(c) = s("color") {
                vars.push(("download-color".into(), c));
            }
        }
        _ => {}
    }
    let ovr: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let mut out = strip_css(&resolve_vars(src, &ovr));
    // colours also go to the presentation attributes (the helper sets both)
    for (k, v) in &vars {
        let (part, attr) = match k.as_str() {
            "light-fill" => ("light-glass", "fill"),
            "mute-color" => ("mute-mark", "stroke"),
            "download-color" => ("download-arrow", "stroke"),
            "thermostat-color" => ("thermostat-ring", "stroke"),
            "pomodoro-color" => ("pomodoro-ring", "stroke"),
            "clock-color" => ("clock-ring", "stroke"),
            "clock-fill" => ("clock-dial", "fill"),
            "thermostat-number-color" => ("thermostat-temperature", "fill"),
            other if other.starts_with("spool-") => {
                let n = other.trim_start_matches("spool-").trim_end_matches("-color");
                let part = format!("spool-{n}-ring");
                set_part_attr(&mut out, &part, "stroke", v);
                continue;
            }
            _ => continue,
        };
        set_part_attr(&mut out, part, attr, v);
        if k == "thermostat-color" {
            set_part_attr(&mut out, "thermostat-core", "stroke", v);
        }
        if k == "pomodoro-color" {
            set_part_attr(&mut out, "pomodoro-hands", "stroke", v);
        }
    }
    for (part, p) in dash {
        let marker = format!("data-part=\"{part}\"");
        let circ = out
            .find(&marker)
            .and_then(|m| {
                let start = out[..m].rfind('<')?;
                let end = m + out[m..].find('>')?;
                let tag = &out[start..end];
                let i = tag.find("data-circumference=\"")? + 20;
                tag[i..i + tag[i..].find('"')?].parse::<f64>().ok()
            })
            .unwrap_or(100.0);
        set_part_attr(&mut out, &part, "stroke-dasharray", &format!("{:.4} {:.4}", p * circ / 100.0, circ));
    }
    for (h, a) in hands {
        set_part_attr(&mut out, &format!("clock-{h}"), "transform", &format!("rotate({a:.3} 32 32)"));
    }
    if let Some(t) = temp {
        let v = (t * 10.0).round() / 10.0;
        let text = if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v:.1}") };
        let marker = "data-part=\"thermostat-temperature\"";
        if let Some(m) = out.find(marker) {
            if let (Some(gt), Some(close)) = (out[m..].find('>'), out[m..].find("</text>")) {
                out.replace_range(m + gt + 1..m + close, &text);
            }
        }
        let size = (58.0 / text.chars().count() as f64).min(25.0);
        set_part_attr(&mut out, "thermostat-temperature", "font-size", &format!("{size:.2}"));
    }
    out
}

type Key = (String, u32, String);
static CACHE: OnceLock<Mutex<HashMap<Key, Option<Arc<Pixmap>>>>> = OnceLock::new();

/// Rasterise icon `name` at `size` × `size` pixels.
pub fn render(name: &str, size: u32, overrides: &[(&str, &str)]) -> Option<Arc<Pixmap>> {
    render_with(name, size, overrides, None)
}

/// Rasterise a stateful icon with a state object (falls back to plain icons).
pub fn render_state(name: &str, size: u32, overrides: &[(&str, &str)], state: &serde_json::Value) -> Option<Arc<Pixmap>> {
    render_with(name, size, overrides, Some(state))
}

fn render_with(name: &str, size: u32, overrides: &[(&str, &str)], state: Option<&serde_json::Value>) -> Option<Arc<Pixmap>> {
    let key = (name.to_string(), size, format!("{overrides:?}{}", state.map(|s| s.to_string()).unwrap_or_default()));
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(v) = cache.lock().unwrap().get(&key) {
        return v.clone();
    }
    let src = svg(name)?;
    let text = match state {
        Some(st) if is_stateful(name) => resolve_vars(&apply_state(name, src, st), overrides),
        _ => strip_css(&resolve_vars(src, overrides)),
    };
    let pm = rasterise(&text, size).map(Arc::new);
    let mut c = cache.lock().unwrap();
    if c.len() > 800 {
        c.clear();
    }
    c.insert(key, pm.clone());
    pm
}

/// System fonts for SVG text (the thermostat's number).
fn options() -> &'static resvg::usvg::Options<'static> {
    static OPT: OnceLock<resvg::usvg::Options<'static>> = OnceLock::new();
    OPT.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        for p in [
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
            "/System/Library/Fonts/SFNS.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ] {
            if let Ok(data) = std::fs::read(p) {
                db.load_font_data(data);
            }
        }
        let fam = db.faces().next().and_then(|f| f.families.first().map(|x| x.0.clone()));
        if let Some(fam) = fam {
            db.set_sans_serif_family(fam.clone());
            db.set_serif_family(fam);
        }
        let mut o = resvg::usvg::Options::default();
        o.fontdb = Arc::new(db);
        o
    })
}

fn rasterise(text: &str, size: u32) -> Option<Pixmap> {
    let tree = resvg::usvg::Tree::from_str(text, options()).ok()?;
    let mut pm = resvg::tiny_skia::Pixmap::new(size, size)?;
    let s = size as f32 / tree.size().width().max(tree.size().height());
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(s, s), &mut pm.as_mut());
    // resvg may use another tiny-skia version than the canvas; move the bytes across.
    Pixmap::from_vec(pm.take(), tiny_skia::IntSize::from_wh(size, size)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_icons_render() {
        assert_eq!(ICONS.len(), 218);
        for (n, _) in ICONS {
            let pm = render(n, 64, &[]).unwrap_or_else(|| panic!("{n} does not render"));
            assert!(pm.data().chunks_exact(4).any(|p| p[3] > 0), "{n} is empty");
        }
    }

    #[test]
    fn colours_survive_and_override() {
        // mint parts of "play" render mint, and can be recoloured
        let mint = render("play", 64, &[]).unwrap();
        let has = |pm: &Pixmap, rgb: [u8; 3]| {
            pm.data().chunks_exact(4).any(|p| p[3] == 255 && p[0] == rgb[0] && p[1] == rgb[1] && p[2] == rgb[2])
        };
        assert!(has(&mint, [0x69, 0xE0, 0xC1]), "mint lost: CSS colours not applied");
        let red = render("play", 64, &[("icon-mint", "#FF0000")]).unwrap();
        assert!(has(&red, [255, 0, 0]));
        assert_eq!(resolve_vars("stroke:var(--a,#123)", &[]), "stroke:#123");
    }

    fn count(pm: &Pixmap, rgb: [u8; 3]) -> usize {
        pm.data().chunks_exact(4).filter(|p| p[3] > 200 && (0..3).all(|i| (p[i] as i32 - rgb[i] as i32).abs() < 30)).count()
    }

    #[test]
    fn stateful_icons() {
        use serde_json::json;
        // light: colour and off
        let on = render_state("light", 64, &[], &json!({"on": true, "color": "#FF0000"})).unwrap();
        let off = render_state("light", 64, &[], &json!({"on": false})).unwrap();
        assert!(count(&on, [255, 0, 0]) > 200 && count(&off, [255, 0, 0]) == 0);
        // pomodoro ring progress: more amber at 90 % than at 10 %
        let amber = [0xFF, 0xBF, 0x69];
        let p10 = render_state("pomodoro", 128, &[], &json!({"progress": 10})).unwrap();
        let p90 = render_state("pomodoro", 128, &[], &json!({"progress": 90})).unwrap();
        assert!(count(&p90, amber) > count(&p10, amber) * 3, "{} vs {}", count(&p90, amber), count(&p10, amber));
        // clock hands move
        let a = render_state("analog-clock", 64, &[], &json!({"time": {"hours": 3, "minutes": 0, "seconds": 0}})).unwrap();
        let b = render_state("analog-clock", 64, &[], &json!({"time": {"hours": 9, "minutes": 30, "seconds": 20}})).unwrap();
        assert_ne!(a.data(), b.data());
        // thermostat number is drawn (ivory text) and changes
        let t22 = render_state("thermostat", 128, &[], &json!({"temperature": 22})).unwrap();
        let t185 = render_state("thermostat", 128, &[], &json!({"temperature": 18.5, "color": "#69E0C1"})).unwrap();
        assert!(count(&t22, [0xF4, 0xF1, 0xE8]) > 150, "no number drawn");
        assert_ne!(t22.data(), t185.data());
        // AMS spools take their colours
        let ams = render_state("bambu-ams", 128, &[], &json!({"spools": [{"color": "#0000FF", "progress": 100}]})).unwrap();
        assert!(count(&ams, [0, 0, 255]) > 30);
        // a plain render of a stateful icon still works (defaults)
        assert!(render("analog-clock", 64, &[]).is_some());
    }
}
