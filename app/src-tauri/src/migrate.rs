//! deck.json migrations. Each step takes the JSON of one version and returns the next.
//! Unknown fields are carried along untouched.
//!
//! Version 1 (no `version` field) to 2:
//! - `"app": {"type": "sysmon", "modes": [...]}` becomes `"app": "core:sysmon", "config": {"modes": [...]}`
//! - `"actions": {"press": [{"type": "page", "page": "x"}]}` becomes
//!   `"on": {"press": [{"action": "core:page", "params": {"page": "x"}}]}` (keys and status key)
//! - Night Signal icon names get the core namespace: `"volume-on"` becomes `"core:volume-on"`
//! - `icon_state` becomes `props`, keyed by interface property (`legacy_props`)

use serde_json::{json, Map, Value};

/// Version of a parsed deck.json: the `version` field, 1 when missing.
pub fn version(v: &Value) -> u64 {
    v.get("version").and_then(Value::as_u64).unwrap_or(1)
}

/// Bring `v` up to `target`. Err for a version newer than this app knows.
pub fn migrate(mut v: Value, target: u32) -> Result<Value, String> {
    let mut ver = version(&v);
    if ver > target as u64 {
        return Err(format!("deck.json is version {ver}; this app reads up to version {target}"));
    }
    while ver < target as u64 {
        v = match ver {
            1 => v1_to_v2(v),
            _ => return Err(format!("no migration from version {ver}")),
        };
        ver += 1;
        v["version"] = json!(ver);
    }
    Ok(v)
}

fn v1_to_v2(mut v: Value) -> Value {
    if let Some(sk) = v.get_mut("status_key").and_then(Value::as_object_mut) {
        if let Some(a) = sk.remove("actions") {
            sk.insert("on".into(), actions(a));
        }
    }
    for p in v.get_mut("profiles").and_then(Value::as_array_mut).into_iter().flatten() {
        for page in p.get_mut("pages").and_then(Value::as_object_mut).into_iter().flat_map(|m| m.values_mut()) {
            for key in page.as_object_mut().into_iter().flat_map(|m| m.values_mut()) {
                if let Some(k) = key.as_object_mut() {
                    migrate_key(k);
                }
            }
        }
    }
    v
}

fn migrate_key(k: &mut Map<String, Value>) {
    if let Some(Value::Object(app)) = k.get_mut("app") {
        let mut app = std::mem::take(app);
        let kind = app.remove("type").and_then(|t| t.as_str().map(str::to_string)).unwrap_or_default();
        k.insert("app".into(), json!(format!("core:{kind}")));
        if !app.is_empty() {
            k.insert("config".into(), Value::Object(app));
        }
    }
    for st in k.get_mut("states").and_then(Value::as_array_mut).into_iter().flatten() {
        let Some(st) = st.as_object_mut() else { continue };
        if let Some(a) = st.remove("actions") {
            st.insert("on".into(), actions(a));
        }
        let icon = st.get("icon").and_then(Value::as_str).map(str::to_string);
        if let Some(i) = &icon {
            if crate::icons::exists(i) {
                st.insert("icon".into(), json!(format!("core:{}", i.trim())));
            }
        }
        if let Some(state) = st.remove("icon_state") {
            let props = legacy_props(icon.as_deref().unwrap_or(""), &state);
            if !props.is_empty() {
                st.insert("props".into(), Value::Object(props));
            }
        }
    }
}

fn actions(a: Value) -> Value {
    let Value::Object(mut ev) = a else { return a };
    for list in ev.values_mut() {
        for item in list.as_array_mut().into_iter().flatten() {
            if let Value::Object(mut obj) = std::mem::take(item) {
                let kind = obj.remove("type").and_then(|t| t.as_str().map(str::to_string)).unwrap_or_default();
                let mut out = Map::new();
                out.insert("action".into(), json!(format!("core:{kind}")));
                if !obj.is_empty() {
                    out.insert("params".into(), Value::Object(obj));
                }
                *item = Value::Object(out);
            }
        }
    }
    Value::Object(ev)
}

/// The old state object of a stateful icon (`setIconState` shape) as interface
/// properties: {"on": true, "color": "#F00"} -> {"toggle.on": true, "rgb.color": "#F00"}.
pub fn legacy_props(icon: &str, state: &Value) -> Map<String, Value> {
    let mut p = Map::new();
    let Some(s) = state.as_object() else { return p };
    if let Some(v) = s.get("on") {
        p.insert("toggle.on".into(), v.clone());
    }
    if let Some(v) = s.get("color") {
        p.insert("rgb.color".into(), v.clone());
    }
    if let Some(v) = s.get("progress") {
        p.insert("progress.value".into(), v.clone());
    }
    if let Some(t) = s.get("temperature").and_then(Value::as_f64) {
        let t = (t * 10.0).round() / 10.0;
        p.insert("label.text".into(), json!(if t.fract() == 0.0 { format!("{t:.0}") } else { format!("{t:.1}") }));
    }
    if let Some(t) = s.get("time") {
        let g = |k: &str| t.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let (h, m, sec) = (g("hours"), g("minutes"), g("seconds"));
        p.insert("clock.hour_angle".into(), json!((h % 12.0) * 30.0 + m * 0.5 + sec / 120.0));
        p.insert("clock.minute_angle".into(), json!(m * 6.0 + sec * 0.1));
        p.insert("clock.second_angle".into(), json!(sec * 6.0));
    }
    if icon.trim().trim_start_matches("core:").starts_with("bambu-ams") {
        for (i, sp) in s.get("spools").and_then(Value::as_array).into_iter().flatten().take(4).enumerate() {
            if let Some(c) = sp.get("color") {
                p.insert(format!("com.ricktap.bambu:ams.slot{}_color", i + 1), c.clone());
            }
            if let Some(r) = sp.get("progress") {
                p.insert(format!("com.ricktap.bambu:ams.slot{}_remaining", i + 1), r.clone());
            }
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_keys_and_actions() {
        let v1 = json!({
            "status_key": {"actions": {"press": [{"type": "home"}]}},
            "profiles": [{"name": "Default", "pages": {"main": {
                "0": {"app": {"type": "sysmon", "modes": ["cpu"]}},
                "1": {"states": [{"icon": "light", "icon_state": {"on": true, "color": "#F00"},
                                  "actions": {"long": [{"type": "page", "page": "x"}]}}]},
                "2": {"states": [{"icon": "🎵", "label": "x"}]},
                "3": {"app": "com.x:y", "future": 1}
            }}}]
        });
        let v2 = migrate(v1, 2).unwrap();
        assert_eq!(v2["version"], 2);
        assert_eq!(v2["status_key"]["on"]["press"][0], json!({"action": "core:home"}));
        let keys = &v2["profiles"][0]["pages"]["main"];
        assert_eq!(keys["0"], json!({"app": "core:sysmon", "config": {"modes": ["cpu"]}}));
        assert_eq!(keys["1"]["states"][0]["icon"], "core:light");
        assert_eq!(keys["1"]["states"][0]["props"], json!({"toggle.on": true, "rgb.color": "#F00"}));
        assert_eq!(keys["1"]["states"][0]["on"]["long"][0], json!({"action": "core:page", "params": {"page": "x"}}));
        assert_eq!(keys["2"]["states"][0]["icon"], "🎵");
        assert_eq!(keys["3"], json!({"app": "com.x:y", "future": 1}), "keys already in the new shape stay");
    }

    #[test]
    fn newer_versions_are_refused() {
        assert!(migrate(json!({"version": 9, "profiles": []}), 2).is_err());
        assert_eq!(migrate(json!({"version": 2, "x": 1}), 2).unwrap(), json!({"version": 2, "x": 1}));
    }
}
