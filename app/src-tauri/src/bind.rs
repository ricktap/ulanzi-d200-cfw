//! Signals and the binding language (docs/deckling/architecture.md, section 6).
//!
//! A binding is a JSON value. Plain values stand for themselves. An object with one
//! source key is resolved:
//!
//! | key      | reads                                          | allowed in        |
//! |----------|------------------------------------------------|-------------------|
//! | `bind`   | a signal; `{name}` placeholders from settings  | app, icon, theme  |
//! | `config` | a setting of the key                           | app               |
//! | `prop`   | `<interface>.<property>` of the key's app      | icon              |
//! | `tone`   | a theme variable                               | icon              |
//!
//! then `fallback` (when the value is missing), `steps` (with optional `range`), `map`, `prefix`/`suffix`.
//! Values in `steps`, `fallback` and theme variables may be bindings themselves.

use serde_json::{Map, Value};
use std::collections::{BTreeSet, HashMap};

/// The core theme variables. A `tone` on any other variable needs a `fallback`.
pub const CORE_VARIABLES: [&str; 4] = ["background", "base", "muted", "accent"];

/// Latest value per signal.
#[derive(Debug, Default, Clone)]
pub struct Signals {
    values: HashMap<String, Value>,
    /// Bumped whenever a value changes.
    pub generation: u64,
    changed: BTreeSet<String>,
}

impl Signals {
    /// Set a signal. Returns true when the value changed.
    pub fn publish(&mut self, name: &str, value: Value) -> bool {
        if self.values.get(name) == Some(&value) {
            return false;
        }
        self.values.insert(name.to_string(), value);
        self.generation += 1;
        self.changed.insert(name.to_string());
        true
    }
    /// Names changed since the last call.
    pub fn take_changed(&mut self) -> BTreeSet<String> {
        std::mem::take(&mut self.changed)
    }
    pub fn clear(&mut self, name: &str) -> bool {
        let had = self.values.remove(name).is_some();
        if had {
            self.generation += 1;
            self.changed.insert(name.to_string());
        }
        had
    }
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }
    pub fn all(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.values.iter()
    }
}

/// Everything a binding can read, for one key.
pub struct Ctx<'a> {
    /// The app's `implements`: interface -> property -> binding.
    pub implements: Option<&'a Map<String, Value>>,
    /// Static props ("toggle.on" -> true) for keys without an app, remote keys, cards.
    pub props: Option<&'a Map<String, Value>>,
    /// The key's app settings.
    pub config: &'a Map<String, Value>,
    pub signals: &'a Signals,
    /// Theme variables, after overlays.
    pub theme: &'a Map<String, Value>,
}

/// What a resolution read; used to redraw a key only when one of these changes.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Reads {
    pub signals: BTreeSet<String>,
    pub variables: BTreeSet<String>,
}

const DEPTH: u32 = 16;

pub fn resolve(spec: &Value, ctx: &Ctx, reads: &mut Reads) -> Option<Value> {
    resolve_d(spec, ctx, reads, 0)
}

fn resolve_d(spec: &Value, ctx: &Ctx, reads: &mut Reads, depth: u32) -> Option<Value> {
    let Value::Object(o) = spec else {
        return if spec.is_null() { None } else { Some(spec.clone()) };
    };
    if depth > DEPTH {
        return None;
    }
    let value = if let Some(name) = o.get("tone").and_then(Value::as_str) {
        reads.variables.insert(name.to_string());
        ctx.theme.get(name).cloned().and_then(|v| resolve_d(&v, ctx, reads, depth + 1))
    } else if let Some(p) = o.get("prop").and_then(Value::as_str) {
        prop(p, ctx, reads, depth)
    } else if let Some(k) = o.get("config").and_then(Value::as_str) {
        ctx.config.get(k).cloned().filter(|v| !v.is_null())
    } else if let Some(b) = o.get("bind").and_then(Value::as_str) {
        let name = fill(b, ctx.config);
        reads.signals.insert(name.clone());
        ctx.signals.get(&name).cloned()
    } else {
        // Not a binding: a plain object value.
        return Some(spec.clone());
    };
    let Some(mut value) = value else {
        return o.get("fallback").and_then(|f| resolve_d(f, ctx, reads, depth + 1));
    };
    if let Some(steps) = o.get("steps").and_then(Value::as_array).filter(|s| !s.is_empty()) {
        let n = number(&value).unwrap_or(0.0);
        let len = steps.len() as i64;
        // With `range: [low, high]` the value picks a step from low to high (clamped);
        // without, the whole number cycles through the steps (time.tick, time.second).
        let range = o.get("range").and_then(Value::as_array).and_then(|r| Some((number(r.first()?)?, number(r.get(1)?)?)));
        let i = match range {
            Some((lo, hi)) if hi > lo => (((n - lo) / (hi - lo) * len as f64).floor() as i64).clamp(0, len - 1),
            _ => (n.floor() as i64).rem_euclid(len),
        } as usize;
        value = resolve_d(&steps[i], ctx, reads, depth + 1)?;
    }
    if let Some(map) = o.get("map").and_then(Value::as_object) {
        value = map.get(&text(&value)).cloned()?;
    }
    if o.contains_key("prefix") || o.contains_key("suffix") {
        let pre = o.get("prefix").map(text).unwrap_or_default();
        let suf = o.get("suffix").map(text).unwrap_or_default();
        value = Value::String(format!("{pre}{}{suf}", text(&value)));
    }
    Some(value)
}

/// `<interface>.<property>`: the app's mapping, else a static prop.
fn prop(p: &str, ctx: &Ctx, reads: &mut Reads, depth: u32) -> Option<Value> {
    let (role, key) = p.rsplit_once('.')?;
    if let Some(spec) = ctx.implements.and_then(|i| i.get(role)).and_then(|r| r.get(key)) {
        return resolve_d(spec, ctx, reads, depth + 1);
    }
    ctx.props.and_then(|m| m.get(p)).cloned().filter(|v| !v.is_null())
}

/// Replace `{key}` with the setting `key`.
pub fn fill(s: &str, config: &Map<String, Value>) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find('}') {
            Some(j) => {
                out.push_str(&config.get(&after[..j]).map(text).unwrap_or_default());
                rest = &after[j + 1..];
            }
            None => {
                out.push_str(&rest[i..]);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

/// A value as text, the way JavaScript would join it into a string.
pub fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", f as i64),
            _ => n.to_string(),
        },
        other => other.to_string(),
    }
}

/// A value as a number: numbers, numeric strings, booleans as 0/1.
pub fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Truthiness, as for `visible`.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty() && s != "false" && s != "0",
        _ => true,
    }
}

/// Load-time check: every `prop`, `bind` and `tone` outside the core variables
/// has a `fallback`. Returns the paths of bindings that miss one.
pub fn missing_fallbacks(spec: &Value, path: &str, out: &mut Vec<String>) {
    match spec {
        Value::Object(o) => {
            let needs = o.contains_key("prop")
                || o.contains_key("bind")
                || o.get("tone").and_then(Value::as_str).is_some_and(|t| !CORE_VARIABLES.contains(&t));
            if needs && !o.contains_key("fallback") {
                out.push(path.to_string());
            }
            for (k, v) in o {
                missing_fallbacks(v, &format!("{path}.{k}"), out);
            }
        }
        Value::Array(a) => {
            for (i, v) in a.iter().enumerate() {
                missing_fallbacks(v, &format!("{path}[{i}]"), out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap()
    }

    struct World {
        implements: Map<String, Value>,
        config: Map<String, Value>,
        signals: Signals,
        theme: Map<String, Value>,
        props: Map<String, Value>,
    }

    impl World {
        fn new() -> World {
            let mut signals = Signals::default();
            signals.publish("com.ricktap.hue:lamp.power", json!(1));
            signals.publish("com.ricktap.hue:lamp.color", json!("hsl(20, 85%, 60%)"));
            signals.publish("com.ricktap.bambu:ams2.slot1.remaining", json!(30));
            signals.publish("time.second", json!(7));
            signals.publish("time.hour_angle", json!(300));
            World {
                implements: obj(json!({
                    "toggle": {"on": {"bind": "com.ricktap.hue:lamp.power", "fallback": false}},
                    "rgb": {"color": {"bind": "com.ricktap.hue:lamp.color", "fallback": "#FFD84D"}},
                    "com.ricktap.bambu:ams": {"slot1_remaining": {"bind": "com.ricktap.bambu:{unit}.slot1.remaining", "fallback": 0}},
                    "clock": {"style": {"config": "display"}, "hour_angle": {"bind": "time.hour_angle", "fallback": 0}}
                })),
                config: obj(json!({"unit": "ams2", "display": "analog"})),
                signals,
                theme: obj(json!({
                    "background": "#0B0D10", "base": "#F4F1E8", "accent": "#69E0C1",
                    "com.ricktap.bambu:ams.led": {"bind": "time.second", "steps": ["#FF4D4D", "#3DDC84"]},
                    "danger": {"tone": "accent"}
                })),
                props: obj(json!({"label.text": "21"})),
            }
        }
        fn ctx(&self) -> Ctx<'_> {
            Ctx { implements: Some(&self.implements), props: Some(&self.props), config: &self.config, signals: &self.signals, theme: &self.theme }
        }
        fn r(&self, spec: Value) -> Option<Value> {
            resolve(&spec, &self.ctx(), &mut Reads::default())
        }
    }

    #[test]
    fn sources() {
        let w = World::new();
        assert_eq!(w.r(json!("#123")), Some(json!("#123")));
        assert_eq!(w.r(json!({"prop": "rgb.color", "fallback": "#000"})), Some(json!("hsl(20, 85%, 60%)")));
        assert_eq!(w.r(json!({"prop": "com.ricktap.bambu:ams.slot1_remaining", "fallback": 0})), Some(json!(30)));
        assert_eq!(w.r(json!({"prop": "clock.style"})), Some(json!("analog")));
        assert_eq!(w.r(json!({"tone": "base"})), Some(json!("#F4F1E8")));
        assert_eq!(w.r(json!({"tone": "danger", "fallback": "#F00"})), Some(json!("#69E0C1")), "variables chain");
        assert_eq!(w.r(json!({"bind": "time.second"})), Some(json!(7)));
        assert_eq!(w.r(json!({"prop": "label.text", "fallback": ""})), Some(json!("21")), "static props");
    }

    #[test]
    fn fallback_when_missing() {
        let w = World::new();
        assert_eq!(w.r(json!({"prop": "progress.value", "fallback": 0})), Some(json!(0)));
        assert_eq!(w.r(json!({"bind": "nope", "fallback": {"tone": "accent"}})), Some(json!("#69E0C1")));
        assert_eq!(w.r(json!({"tone": "com.x:y", "fallback": "#FFF"})), Some(json!("#FFF")));
        assert_eq!(w.r(json!({"bind": "nope"})), None);
        // a property the interface no longer has: the fallback, not an error
        assert_eq!(w.r(json!({"prop": "rgb.colour", "fallback": "#FFD84D"})), Some(json!("#FFD84D")));
    }

    #[test]
    fn steps_map_prefix_suffix() {
        let w = World::new();
        // theme variable bound to a signal with steps: 7 % 2 = 1
        assert_eq!(w.r(json!({"tone": "com.ricktap.bambu:ams.led", "fallback": "#000"})), Some(json!("#3DDC84")));
        assert_eq!(w.r(json!({"prop": "toggle.on", "fallback": false, "steps": [{"tone": "muted", "fallback": "#555"}, {"tone": "accent"}]})), Some(json!("#69E0C1")));
        assert_eq!(w.r(json!({"prop": "clock.style", "map": {"analog": "visible", "digital": "hidden"}})), Some(json!("visible")));
        assert_eq!(
            w.r(json!({"prop": "clock.hour_angle", "fallback": 0, "prefix": "rotate(", "suffix": " 32 32)"})),
            Some(json!("rotate(300 32 32)"))
        );
        // negative numbers wrap around
        assert_eq!(w.r(json!({"bind": "time.second", "steps": ["a", "b", "c"]})), Some(json!("b")));
        // with a range the value picks from low to high and is clamped
        let r = |n: f64| {
            let mut w = World::new();
            w.signals.publish("x", json!(n));
            w.r(json!({"bind": "x", "range": [0, 100], "steps": ["low", "mid", "high"]}))
        };
        assert_eq!((r(0.0), r(40.0), r(80.0), r(100.0), r(-5.0), r(250.0)),
            (Some(json!("low")), Some(json!("mid")), Some(json!("high")), Some(json!("high")), Some(json!("low")), Some(json!("high"))));
    }

    #[test]
    fn records_reads() {
        let w = World::new();
        let mut reads = Reads::default();
        resolve(&json!({"prop": "com.ricktap.bambu:ams.slot1_remaining", "fallback": 0}), &w.ctx(), &mut reads);
        resolve(&json!({"tone": "com.ricktap.bambu:ams.led", "fallback": "#000"}), &w.ctx(), &mut reads);
        assert!(reads.signals.contains("com.ricktap.bambu:ams2.slot1.remaining"));
        assert!(reads.signals.contains("time.second"));
        assert!(reads.variables.contains("com.ricktap.bambu:ams.led"));
    }

    #[test]
    fn checks_fallbacks() {
        let mut out = vec![];
        missing_fallbacks(
            &json!({"glass": {"fill": {"prop": "rgb.color"}, "stroke": {"tone": "base"}, "x": {"tone": "danger"}}}),
            "parts",
            &mut out,
        );
        assert_eq!(out, vec!["parts.glass.fill".to_string(), "parts.glass.x".to_string()]);
    }

    #[test]
    fn signals_report_changes() {
        let mut s = Signals::default();
        assert!(s.publish("a", json!(1)));
        assert!(!s.publish("a", json!(1)));
        assert!(s.publish("a", json!(2)));
        assert_eq!(s.generation, 2);
    }
}
