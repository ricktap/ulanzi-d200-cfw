//! The bundled "Night Signal" icon set (215 SVGs, 64 × 64, line style).
//! An icon is referenced by name in the config, e.g. "icon": "volume-on".

use crate::bind::{self, Ctx, Reads};
use crate::registry::IconDef;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tiny_skia::Pixmap;

include!(concat!(env!("OUT_DIR"), "/icons_gen.rs"));

/// Night Signal icon by name, with or without its namespace (`com.deckling.core:`, or the old `core:`).
pub fn svg(name: &str) -> Option<&'static str> {
    let name = name.trim();
    let name = name.strip_prefix("com.deckling.core:").or_else(|| name.strip_prefix("core:")).unwrap_or(name);
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

/// Elements whose `data-part` list contains `part`: (tag start, tag end) byte ranges.
fn part_tags(svg: &str, part: &str) -> Vec<(usize, usize)> {
    let mut out = vec![];
    for (i, _) in svg.match_indices("data-part=\"") {
        let rest = &svg[i + 11..];
        let names = &rest[..rest.find('"').unwrap_or(0)];
        if names.split_whitespace().any(|n| n == part) {
            let start = svg[..i].rfind('<').unwrap_or(0);
            let end = i + svg[i..].find('>').unwrap_or(0);
            out.push((start, end));
        }
    }
    out
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let i = tag.find(&needle)? + needle.len();
    Some(&tag[i..i + tag[i..].find('"')?])
}

/// `tag` with attribute `name` set (None: removed), and the same CSS property
/// dropped from its inline style so the attribute wins.
fn with_attr(tag: &str, name: &str, value: Option<&str>) -> String {
    let mut t = tag.to_string();
    let needle = format!(" {name}=\"");
    if let Some(i) = t.find(&needle) {
        let vs = i + needle.len();
        let ve = vs + t[vs..].find('"').unwrap_or(0);
        t.replace_range(i..ve + 1, "");
    }
    if let Some(st) = attr(&t, "style").map(str::to_string) {
        let kept: Vec<&str> = st.split(';').map(str::trim).filter(|d| !d.is_empty() && d.split(':').next().unwrap_or("").trim() != name).collect();
        t = with_attr_raw(&t, "style", if kept.is_empty() { None } else { Some(kept.join(";")) });
    }
    match value {
        Some(v) => {
            let closing = if t.ends_with('/') { "/" } else { "" };
            format!("{} {name}=\"{}\"{closing}", t.trim_end_matches('/'), escape(v))
        }
        None => t,
    }
}

fn with_attr_raw(tag: &str, name: &str, value: Option<String>) -> String {
    let needle = format!(" {name}=\"");
    let mut t = tag.to_string();
    if let Some(i) = t.find(&needle) {
        let vs = i + needle.len();
        let ve = vs + t[vs..].find('"').unwrap_or(0);
        t.replace_range(i..ve + 1, "");
    }
    match value {
        Some(v) => {
            let closing = if t.ends_with('/') { "/" } else { "" };
            format!("{} {name}=\"{v}\"{closing}", t.trim_end_matches('/'))
        }
        None => t,
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;")
}

/// Apply an icon's bindings to its SVG (architecture section 7):
/// `progress` fills a stroke 0..100, `visible` shows or hides, `text` sets the
/// content, any other key is set as an SVG attribute.
pub fn apply_parts(def: &IconDef, ctx: &Ctx, reads: &mut Reads) -> String {
    let mut out = strip_css(&def.svg);
    for (part, attrs) in &def.parts {
        let Some(attrs) = attrs.as_object() else { continue };
        for (key, spec) in attrs {
            let Some(v) = bind::resolve(spec, ctx, reads) else { continue };
            // Edit from the back so earlier ranges stay valid.
            for (start, end) in part_tags(&out, part).into_iter().rev() {
                let tag = out[start..end].to_string();
                match key.as_str() {
                    "progress" => {
                        let p = bind::number(&v).unwrap_or(0.0).clamp(0.0, 100.0);
                        // resvg ignores pathLength; the ring's length is in data-circumference.
                        let len = attr(&tag, "data-circumference").and_then(|c| c.parse::<f64>().ok()).unwrap_or(100.0);
                        let t = with_attr(&tag, "pathLength", None);
                        let t = with_attr(&t, "stroke-dasharray", Some(&format!("{:.4} {:.4}", p * len / 100.0, len)));
                        out.replace_range(start..end, &t);
                    }
                    "visible" => {
                        let t = with_attr(&tag, "visibility", Some(if bind::truthy(&v) { "visible" } else { "hidden" }));
                        out.replace_range(start..end, &t);
                    }
                    "text" => {
                        let close = out[end..].find("</").map(|c| end + c);
                        if let Some(close) = close {
                            out.replace_range(end + 1..close, &escape(&bind::text(&v)));
                        }
                    }
                    _ => {
                        let t = with_attr(&tag, key, Some(&bind::text(&v)));
                        out.replace_range(start..end, &t);
                    }
                }
            }
        }
    }
    out
}

/// Rasterise a registry icon with its bindings resolved in `ctx`.
pub fn render_bound(def: &IconDef, size: u32, overrides: &[(&str, &str)], ctx: &Ctx, reads: &mut Reads) -> Option<Arc<Pixmap>> {
    if def.parts.is_empty() {
        // An icon without bound parts: its own SVG (icon sets other than Night Signal), else by name.
        if !def.svg.is_empty() {
            return cached(strip_css(&resolve_vars(&def.svg, overrides)), size);
        }
        return render(&def.id, size, overrides);
    }
    let text = resolve_vars(&apply_parts(def, ctx, reads), overrides);
    cached(text, size)
}

fn cached(text: String, size: u32) -> Option<Arc<Pixmap>> {
    let key = (text, size, String::new());
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(v) = cache.lock().unwrap().get(&key) {
        return v.clone();
    }
    let pm = rasterise(&key.0, size).map(Arc::new);
    let mut c = cache.lock().unwrap();
    if c.len() > 800 {
        c.clear();
    }
    c.insert(key, pm.clone());
    pm
}

type Key = (String, u32, String);
static CACHE: OnceLock<Mutex<HashMap<Key, Option<Arc<Pixmap>>>>> = OnceLock::new();

/// Rasterise Night Signal icon `name` at `size` × `size` pixels, without bindings.
pub fn render(name: &str, size: u32, overrides: &[(&str, &str)]) -> Option<Arc<Pixmap>> {
    let src = svg(name)?;
    cached(strip_css(&resolve_vars(src, overrides)), size)
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
        assert_eq!(ICONS.len(), 219);
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

    /// The eight stateful icons, now driven by their binding files.
    #[test]
    fn every_registered_icon_draws() {
        // every icon of every built-in package, bound or not, with no values (fallbacks)
        let reg = crate::registry::builtin();
        let (sig, empty) = (bind::Signals::default(), serde_json::Map::new());
        let ctx = Ctx { implements: None, props: None, config: &empty, signals: &sig, theme: &empty };
        let failed: Vec<&String> = reg.icons.keys().filter(|id| render_bound(&reg.icons[*id], 48, &[], &ctx, &mut Reads::default()).is_none()).collect();
        assert!(failed.is_empty(), "icons that do not draw: {failed:?}");
    }

    #[test]
    fn stateful_icons() {
        use serde_json::{json, Map, Value};
        let reg = crate::registry::builtin();
        let sig = bind::Signals::default();
        let theme: Map<String, Value> = json!({"accent": "#69E0C1", "base": "#F4F1E8", "muted": "#87909E", "background": "#11171D"})
            .as_object()
            .cloned()
            .unwrap();
        let cfg = Map::new();
        let draw = |icon: &str, size: u32, props: Value| {
            let props = props.as_object().cloned().unwrap();
            let ctx = Ctx { implements: None, props: Some(&props), config: &cfg, signals: &sig, theme: &theme };
            render_bound(reg.icon(icon).unwrap(), size, &[], &ctx, &mut Reads::default()).unwrap()
        };
        // light: colour and off
        let on = draw("com.deckling.core:light", 64, json!({"toggle.on": true, "rgb.color": "#FF0000"}));
        let off = draw("com.deckling.core:light", 64, json!({"toggle.on": false, "rgb.color": "#FF0000"}));
        assert!(count(&on, [255, 0, 0]) > 200 && count(&off, [255, 0, 0]) == 0);
        // pomodoro ring progress: more amber at 90 % than at 10 %
        let amber = [0xFF, 0xBF, 0x69];
        let p10 = draw("com.deckling.core:pomodoro", 128, json!({"progress.value": 10}));
        let p90 = draw("com.deckling.core:pomodoro", 128, json!({"progress.value": 90}));
        assert!(count(&p90, amber) > count(&p10, amber) * 3, "{} vs {}", count(&p90, amber), count(&p10, amber));
        // clock hands move
        let a = draw("com.deckling.core:analog-clock", 64, json!({"clock.hour_angle": 90, "clock.minute_angle": 0}));
        let b = draw("com.deckling.core:analog-clock", 64, json!({"clock.hour_angle": 285, "clock.minute_angle": 180}));
        assert_ne!(a.data(), b.data());
        // thermostat number is drawn (ivory text) and changes
        let t22 = draw("com.deckling.core:thermostat", 128, json!({"label.text": "22"}));
        let t185 = draw("com.deckling.core:thermostat", 128, json!({"label.text": "18.5"}));
        assert!(count(&t22, [0xF4, 0xF1, 0xE8]) > 150, "no number drawn");
        assert_ne!(t22.data(), t185.data());
        // AMS spools take their colours from another plugin's interface
        let ams = draw("com.deckling.core:bambu-ams", 128, json!({"com.ricktap.bambu:ams.slot1_color": "#0000FF", "com.ricktap.bambu:ams.slot1_remaining": 100}));
        assert!(count(&ams, [0, 0, 255]) > 30);
        // downloads: muted grey off, accent on
        let idle = draw("com.deckling.core:downloads", 64, json!({}));
        let busy = draw("com.deckling.core:downloads", 64, json!({"toggle.on": true}));
        assert!(count(&busy, [0x69, 0xE0, 0xC1]) > count(&idle, [0x69, 0xE0, 0xC1]));
        // without any values the fallbacks draw the old defaults
        assert!(count(&draw("com.deckling.core:light", 64, json!({})), [0x69, 0xE0, 0xC1]) > 200);
    }

    /// A theme variable bound to a signal recolours an icon; no plugin sets a colour.
    #[test]
    fn theme_variable_bound_to_signal() {
        use serde_json::{json, Map, Value};
        let reg = crate::registry::builtin();
        let mut sig = bind::Signals::default();
        let theme: Map<String, Value> = json!({"accent": {"bind": "time.second", "steps": ["#FF0000", "#0000FF"], "fallback": "#69E0C1"}})
            .as_object()
            .cloned()
            .unwrap();
        let cfg = Map::new();
        let mut draw = |s: i64| {
            sig.publish("time.second", json!(s));
            let ctx = Ctx { implements: None, props: None, config: &cfg, signals: &sig, theme: &theme };
            let mut reads = Reads::default();
            let pm = render_bound(reg.icon("com.deckling.core:light").unwrap(), 64, &[], &ctx, &mut reads).unwrap();
            assert!(reads.signals.contains("time.second"));
            pm
        };
        assert!(count(&draw(0), [255, 0, 0]) > 200);
        assert!(count(&draw(1), [0, 0, 255]) > 200);
    }
}
