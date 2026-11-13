//! deck.json migrations. Each step takes the JSON of one version and returns the next.
//! Unknown fields are carried along untouched.
//!
//! Version 1 (no `version` field) to 2:
//! - `"app": {"type": "sysmon", "modes": [...]}` becomes `"app": "core:sysmon", "config": {"modes": [...]}`
//! - `"actions": {"press": [{"type": "page", "page": "x"}]}` becomes
//!   `"on": {"press": [{"action": "core:page", "params": {"page": "x"}}]}` (keys and status key)
//! - Night Signal icon names get the core namespace: `"volume-on"` becomes `"core:volume-on"`
//! - `icon_state` becomes `props`, keyed by interface property (`legacy_props`)
//!
//! Version 2 to 3: `core:` keeps only what is fixed in the app (navigation, states,
//! brightness, notifications, the plain button). Everything loaded as a plugin moves
//! to its own namespace: `com.deckling.core` (built-in apps, Night Signal icons),
//! `com.deckling.teams`, `com.deckling.jetbrains`, `com.deckling.ha`. App ids, action
//! ids, icon ids and signal names in bindings are renamed.
//!
//! Version 3 to 4: settings move to the plugins they belong to, as
//! `plugins.<id>.config`: `weather` and `pomodoro` to com.deckling.core, `teams` (its
//! fields) to com.deckling.teams, everything else (long press, profiles following the
//! front app, brightness, MQTT, screensaver, cards, theme) to `core`.
//!
//! Version 6 to 7: a state gets `icon_off` (the icon while the app is off). The per-state
//! icons of Mute, Teams mic and Teams camera (`config.state_icons`, 0.19) become `icon` (on)
//! and `icon_off`.

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
            2 => v2_to_v3(v),
            3 => v3_to_v4(v),
            // 4 to 5 adds the optional `theme` and `overlays` (theme packages); nothing moves.
            4 => v,
            // 5 to 6 adds `on.double` (double press) and `double_press_ms`; nothing moves.
            5 => v,
            6 => v6_to_v7(v),
            _ => return Err(format!("no migration from version {ver}")),
        };
        ver += 1;
        v["version"] = json!(ver);
    }
    Ok(v)
}

const APPS_V3: [(&str, &str); 10] = [
    ("core:sysmon", "com.deckling.core:sysmon"),
    ("core:mute", "com.deckling.core:mute"),
    ("core:weather", "com.deckling.core:weather"),
    ("core:clock", "com.deckling.core:clock"),
    ("core:pomodoro", "com.deckling.core:pomodoro"),
    ("core:media", "com.deckling.core:media"),
    ("core:teams_mute", "com.deckling.teams:mute"),
    ("core:teams_camera", "com.deckling.teams:camera"),
    ("core:teams_presence", "com.deckling.teams:presence"),
    ("core:debugger", "com.deckling.jetbrains:debugger"),
];

const ACTIONS_V3: [(&str, &str); 9] = [
    ("core:command", "com.deckling.core:command"),
    ("core:open", "com.deckling.core:open"),
    ("core:keys", "com.deckling.core:keys"),
    ("core:volume", "com.deckling.core:volume"),
    ("core:media", "com.deckling.core:media"),
    ("core:teams", "com.deckling.teams:meeting"),
    ("core:presence", "com.deckling.teams:presence"),
    ("core:mqtt", "com.deckling.ha:mqtt"),
    ("core:remote", "com.deckling.ha:remote"),
];

/// Signal names of version 2 that moved into a plugin namespace.
fn signal_v3(name: &str) -> Option<String> {
    let fixed = [
        ("teams.connected", "com.deckling.teams:connected"),
        ("teams.in_meeting", "com.deckling.teams:in_meeting"),
        ("teams.mic_on", "com.deckling.teams:mic_on"),
        ("teams.video", "com.deckling.teams:video"),
        ("presence.availability", "com.deckling.teams:presence"),
        ("debugger.active", "com.deckling.jetbrains:debugger.active"),
    ];
    if let Some((_, to)) = fixed.iter().find(|(from, _)| *from == name) {
        return Some(to.to_string());
    }
    ["system.", "audio.", "weather.", "pomodoro.", "media."]
        .iter()
        .any(|p| name.starts_with(p))
        .then(|| format!("com.deckling.core:{name}"))
}

fn rename(v: &mut Value, table: &[(&str, &str)]) {
    if let Some((_, to)) = v.as_str().and_then(|s| table.iter().find(|(from, _)| *from == s)) {
        *v = json!(to);
    }
}

/// Every `"bind"` anywhere (theme variables, props written by hand).
fn rename_binds(v: &mut Value) {
    match v {
        Value::Object(o) => {
            if let Some(new) = o.get("bind").and_then(Value::as_str).and_then(signal_v3) {
                o.insert("bind".into(), json!(new));
            }
            o.values_mut().for_each(rename_binds);
        }
        Value::Array(a) => a.iter_mut().for_each(rename_binds),
        _ => {}
    }
}

fn rename_actions(on: Option<&mut Value>) {
    for list in on.and_then(Value::as_object_mut).into_iter().flat_map(|m| m.values_mut()) {
        for a in list.as_array_mut().into_iter().flatten() {
            if let Some(id) = a.get_mut("action") {
                rename(id, &ACTIONS_V3);
            }
        }
    }
}

fn v2_to_v3(mut v: Value) -> Value {
    rename_binds(&mut v);
    if let Some(sk) = v.get_mut("status_key") {
        rename_actions(sk.get_mut("on"));
    }
    for p in v.get_mut("profiles").and_then(Value::as_array_mut).into_iter().flatten() {
        for page in p.get_mut("pages").and_then(Value::as_object_mut).into_iter().flat_map(|m| m.values_mut()) {
            for key in page.as_object_mut().into_iter().flat_map(|m| m.values_mut()) {
                if let Some(app) = key.get_mut("app") {
                    rename(app, &APPS_V3);
                }
                for st in key.get_mut("states").and_then(Value::as_array_mut).into_iter().flatten() {
                    rename_actions(st.get_mut("on"));
                    if let Some(icon) = st.get_mut("icon") {
                        let new = icon.as_str().and_then(|i| i.strip_prefix("core:")).filter(|n| crate::icons::exists(n)).map(|n| format!("com.deckling.core:{n}"));
                        if let Some(n) = new {
                            *icon = json!(n);
                        }
                    }
                }
            }
        }
    }
    v
}

/// (app, the state that is "on", the state that is "off") for `config.state_icons`.
const ON_OFF_V7: [(&str, &str, &str); 3] =
    [("com.deckling.core:mute", "muted", "on"), ("com.deckling.teams:mute", "live", "muted"), ("com.deckling.teams:camera", "on", "off")];

fn v6_to_v7(mut v: Value) -> Value {
    let profiles = v.get_mut("profiles").and_then(Value::as_array_mut).map(std::mem::take).unwrap_or_default();
    let profiles: Vec<Value> = profiles
        .into_iter()
        .map(|mut p| {
            for page in p.get_mut("pages").and_then(Value::as_object_mut).into_iter().flat_map(|m| m.values_mut()) {
                for key in page.as_object_mut().into_iter().flat_map(|m| m.values_mut()) {
                    let Some((_, on, off)) = ON_OFF_V7.iter().find(|(app, ..)| key["app"] == *app) else { continue };
                    let Some(icons) = key.get_mut("config").and_then(Value::as_object_mut).and_then(|c| c.remove("state_icons")) else { continue };
                    if key["config"].as_object().is_some_and(Map::is_empty) {
                        key.as_object_mut().unwrap().remove("config");
                    }
                    if !key["states"].is_array() || key["states"].as_array().is_some_and(Vec::is_empty) {
                        key["states"] = json!([{}]);
                    }
                    let st = &mut key["states"][0];
                    for (state, field) in [(on, "icon"), (off, "icon_off")] {
                        if let Some(icon) = icons.get(*state).filter(|i| i.is_string()) {
                            st[field] = icon.clone();
                        }
                    }
                }
            }
            p
        })
        .collect();
    if v.get("profiles").is_some() {
        v["profiles"] = Value::Array(profiles);
    }
    v
}

fn v3_to_v4(mut v: Value) -> Value {
    let Some(obj) = v.as_object_mut() else { return v };
    let Some(Value::Object(mut settings)) = obj.remove("settings") else { return v };
    let mut plugins = Map::new();
    let mut put = |id: &str, cfg: Map<String, Value>| {
        if !cfg.is_empty() {
            plugins.insert(id.into(), json!({ "config": cfg }));
        }
    };
    let mut apps = Map::new();
    for k in ["weather", "pomodoro"] {
        if let Some(x) = settings.remove(k) {
            apps.insert(k.into(), x);
        }
    }
    let teams = match settings.remove("teams") {
        Some(Value::Object(t)) => t,
        _ => Map::new(),
    };
    put("core", settings);
    put("com.deckling.core", apps);
    put("com.deckling.teams", teams);
    obj.insert("plugins".into(), Value::Object(plugins));
    v
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
    fn v6_state_icons_become_on_and_off() {
        let v6 = json!({"version": 6, "profiles": [{"name": "Default", "pages": {"main": {
            "1": {"app": "com.deckling.core:mute", "config": {"state_icons": {"muted": "a:x", "on": "a:y"}, "pill": "none"}},
            "2": {"app": "com.deckling.teams:mute", "config": {"state_icons": {"live": "a:mic"}}, "states": [{"label": "Mic"}]},
            "3": {"app": "com.deckling.core:weather", "config": {"state_icons": {"rain": "a:r"}}},
        }}}]});
        let v = migrate(v6, 7).unwrap();
        let keys = &v["profiles"][0]["pages"]["main"];
        assert_eq!(keys["1"], json!({"app": "com.deckling.core:mute", "config": {"pill": "none"}, "states": [{"icon": "a:x", "icon_off": "a:y"}]}));
        assert_eq!(keys["2"], json!({"app": "com.deckling.teams:mute", "states": [{"label": "Mic", "icon": "a:mic"}]}));
        assert_eq!(keys["3"]["config"]["state_icons"]["rain"], "a:r", "weather keeps its states");
        assert_eq!(v["version"], 7);
    }

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
    fn v2_to_plugin_namespaces() {
        let v2 = json!({
            "version": 2,
            "settings": {"theme": {"variables": {"accent": {"bind": "teams.mic_on", "steps": ["#000", "#FFF"], "fallback": "#FFF"}}}},
            "status_key": {"on": {"long": [{"action": "core:remote"}]}},
            "profiles": [{"name": "Default", "pages": {"main": {
                "0": {"app": "core:teams_mute"},
                "1": {"app": "core:sysmon", "states": [{"icon": "core:cpu", "props": {"label.text": {"bind": "system.cpu"}}}]},
                "2": {"states": [{"icon": "core:light", "on": {"press": [{"action": "core:page", "params": {"page": "x"}}, {"action": "core:mqtt", "params": {"topic": "t", "payload": "p"}}]}}]},
                "3": {"app": "com.x:y", "states": [{"icon": "com.x:z", "on": {"press": [{"action": "com.x:go"}]}}]}
            }}}]
        });
        let v3 = migrate(v2, 3).unwrap();
        assert_eq!(v3["version"], 3);
        assert_eq!(v3["settings"]["theme"]["variables"]["accent"]["bind"], "com.deckling.teams:mic_on");
        assert_eq!(v3["status_key"]["on"]["long"][0]["action"], "com.deckling.ha:remote");
        let keys = &v3["profiles"][0]["pages"]["main"];
        assert_eq!(keys["0"]["app"], "com.deckling.teams:mute");
        assert_eq!(keys["1"]["app"], "com.deckling.core:sysmon");
        assert_eq!(keys["1"]["states"][0]["icon"], "com.deckling.core:cpu");
        assert_eq!(keys["1"]["states"][0]["props"]["label.text"]["bind"], "com.deckling.core:system.cpu");
        assert_eq!(keys["2"]["states"][0]["on"]["press"][0]["action"], "core:page", "fixed actions keep core:");
        assert_eq!(keys["2"]["states"][0]["on"]["press"][1]["action"], "com.deckling.ha:mqtt");
        assert_eq!(keys["3"], json!({"app": "com.x:y", "states": [{"icon": "com.x:z", "on": {"press": [{"action": "com.x:go"}]}}]}));
    }

    #[test]
    fn v3_settings_move_to_plugins() {
        let v3 = json!({"version": 3, "settings": {
            "brightness": 90, "mqtt": {"host": "h"}, "weather": {"fahrenheit": true},
            "pomodoro": {"work": 30}, "teams": {"port": 9000}, "future": 1
        }, "profiles": []});
        let v4 = migrate(v3, 4).unwrap();
        assert!(v4.get("settings").is_none());
        let p = &v4["plugins"];
        assert_eq!(p["core"]["config"], json!({"brightness": 90, "mqtt": {"host": "h"}, "future": 1}));
        assert_eq!(p["com.deckling.core"]["config"], json!({"weather": {"fahrenheit": true}, "pomodoro": {"work": 30}}));
        assert_eq!(p["com.deckling.teams"]["config"], json!({"port": 9000}));
    }

    #[test]
    fn newer_versions_are_refused() {
        assert!(migrate(json!({"version": 9, "profiles": []}), 4).is_err());
        assert_eq!(migrate(json!({"version": 4, "x": 1}), 5).unwrap(), json!({"version": 5, "x": 1}));
        assert_eq!(migrate(json!({"version": 4, "x": 1}), 7).unwrap(), json!({"version": 7, "x": 1}));
        assert_eq!(migrate(json!({"version": 4, "x": 1}), 4).unwrap(), json!({"version": 4, "x": 1}));
    }
}
