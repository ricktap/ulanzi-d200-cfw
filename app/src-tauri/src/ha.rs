//! com.deckling.ha: Home Assistant over the app's MQTT connection.
//!
//! Home Assistant describes keys by id, retained, on `<base>/ha/<id>/set`:
//!   {"label": "Kitchen", "kind": "light", "icon": "light", "value": "on",
//!    "on": true, "color": "#FFCC66", "background": "#1F2937", "progress": 40}
//! The plugin publishes each field as a signal `com.deckling.ha:<id>.<field>` and picks
//! the icon: an icon of its own set by `icon` or `kind`, else a Night Signal icon, else
//! the text as given (an emoji). The app `com.deckling.ha:entity` shows one id on a key.
//!
//! The old remote page topics still work: `<base>/remote/<k>/set` is id `remote-<k>`,
//! with `bg` as the colour and `icon_state` translated. `<base>/printer` becomes the
//! `printer.*` signals the printer card reads.

use crate::plugins::{Host, Plugin};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const ID: &str = "com.deckling.ha";

/// Icons of the plugin's own set (plugins/com.deckling.ha/iconset).
pub const ICONS: [&str; 13] =
    ["entity", "light", "switch", "sensor", "climate", "lock", "unlock", "cover", "scene", "vacuum", "automation", "presence", "energy"];

#[derive(Default)]
pub struct Ha {
    /// id -> label, for the id picker.
    ids: BTreeMap<String, String>,
}

impl Ha {
    fn set(&mut self, id: &str, text: &str, host: &mut Host) {
        host.clear_prefix(&format!("{id}."));
        if text.is_empty() {
            self.ids.remove(id);
            return;
        }
        let v: Value = serde_json::from_str(text).unwrap_or_else(|_| json!({ "label": text }));
        let s = |f: &str| {
            v.get(f).and_then(|x| match x {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                Value::Bool(b) => Some(b.to_string()),
                _ => None,
            })
        };
        let state = v.get("icon_state").cloned().unwrap_or(Value::Null);
        let label = s("label").unwrap_or_default();
        host.publish(&format!("{id}.present"), json!(true));
        host.publish(&format!("{id}.label"), json!(label));
        if let Some(x) = s("value").or_else(|| state.get("temperature").map(crate::bind::text)) {
            host.publish(&format!("{id}.value"), json!(x));
        }
        if let Some(on) = v.get("on").or_else(|| state.get("on")).filter(|x| !x.is_null()) {
            host.publish(&format!("{id}.on"), json!(crate::bind::truthy(on)));
        }
        // `color`: the entity's colour (a light), shown by icons with the rgb role.
        if let Some(c) = s("color").or_else(|| state.get("color").and_then(Value::as_str).map(str::to_string)) {
            host.publish(&format!("{id}.color"), json!(c));
        }
        // `background` (old: `bg`): the key's background.
        if let Some(c) = s("background").or_else(|| s("bg")) {
            host.publish(&format!("{id}.background"), json!(c));
        }
        if let Some(p) = v.get("progress").or_else(|| state.get("progress")).and_then(crate::bind::number) {
            host.publish(&format!("{id}.progress"), json!(p.clamp(0.0, 100.0)));
        }
        if let Some(k) = s("kind") {
            host.publish(&format!("{id}.kind"), json!(k));
        }
        host.publish(&format!("{id}.icon"), json!(icon_for(s("icon").as_deref(), s("kind").as_deref())));
        self.ids.insert(id.to_string(), label);
    }

    fn printer(&mut self, text: &str, host: &mut Host) {
        host.clear_prefix("printer.");
        let Ok(v) = serde_json::from_str::<Value>(text) else { return };
        for (k, name) in [("state", "state"), ("name", "name"), ("job", "job"), ("message", "message"), ("ams", "ams")] {
            if let Some(x) = v.get(k).and_then(Value::as_str) {
                host.publish(&format!("printer.{name}"), json!(x));
            }
        }
        if let Some(p) = v.get("percent").and_then(crate::bind::number) {
            host.publish("printer.progress", json!(p));
        }
        if let Some(m) = v.get("remaining_min").and_then(crate::bind::number) {
            host.publish("printer.remaining_min", json!(m));
        }
        for (i, sp) in v.get("spools").and_then(Value::as_array).into_iter().flatten().take(4).enumerate() {
            if let Some(c) = sp.get("color") {
                host.publish(&format!("printer.slot{}_color", i + 1), c.clone());
            }
            if let Some(r) = sp.get("progress") {
                host.publish(&format!("printer.slot{}_remaining", i + 1), r.clone());
            }
        }
    }
}

/// The icon to show: own set by `icon` or `kind`, else Night Signal, else as given.
pub fn icon_for(icon: Option<&str>, kind: Option<&str>) -> String {
    let own = |n: &str| ICONS.contains(&n).then(|| format!("{ID}:{n}"));
    if let Some(i) = icon.map(str::trim).filter(|i| !i.is_empty()) {
        if i.contains(':') {
            return i.to_string();
        }
        if let Some(o) = own(i) {
            return o;
        }
        if crate::icons::exists(i) {
            return format!("com.deckling.core:{i}");
        }
        return i.to_string();
    }
    kind.and_then(own).unwrap_or_else(|| format!("{ID}:entity"))
}

impl Plugin for Ha {
    fn id(&self) -> &'static str {
        ID
    }

    fn mqtt_topics(&self) -> &'static [&'static str] {
        &["ha/+/set", "remote/+/set", "printer"]
    }

    fn on_mqtt(&mut self, topic: &str, payload: &str, host: &mut Host) {
        let parts: Vec<&str> = topic.split('/').collect();
        match parts[..] {
            ["ha", id, "set"] if valid_id(id) => self.set(id, payload, host),
            ["remote", k, "set"] if k.parse::<usize>().is_ok_and(|k| k < crate::config::STATUS_KEY) => {
                self.set(&format!("remote-{k}"), payload, host)
            }
            ["printer"] => self.printer(payload, host),
            _ => {}
        }
    }

    fn on_action(&mut self, name: &str, params: &Map<String, Value>, host: &mut Host) -> Option<Result<(), String>> {
        match name {
            "press" => {
                let id = params.get("id").and_then(Value::as_str).unwrap_or("");
                if !valid_id(id) {
                    return Some(Err("Home Assistant: no entity chosen for this key".into()));
                }
                let event = params.get("event").and_then(Value::as_str).unwrap_or("press");
                match id.strip_prefix("remote-") {
                    Some(k) => host.mqtt(&format!("remote/{k}/action"), event, false),
                    None => host.mqtt(&format!("ha/{id}/action"), event, false),
                }
                Some(Ok(()))
            }
            _ => None,
        }
    }

    fn options(&self, name: &str) -> Option<Vec<Value>> {
        (name == "ids").then(|| {
            self.ids
                .iter()
                .map(|(id, label)| json!({ "label": if label.is_empty() { id.clone() } else { format!("{label} ({id})") }, "value": id }))
                .collect()
        })
    }
}

/// Ids are one topic level: letters, digits, `_`, `-`, `.`.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bind::Signals;

    fn run(f: impl FnOnce(&mut Ha, &mut Host)) -> (Ha, Signals, Vec<(String, String, bool)>) {
        let mut ha = Ha::default();
        let mut s = Signals::default();
        let out = {
            let mut h = Host::new(ID, &mut s, "d200".into());
            f(&mut ha, &mut h);
            h.mqtt_out
        };
        (ha, s, out)
    }

    #[test]
    fn entity_becomes_signals_and_an_id() {
        let (ha, s, _) = run(|ha, h| {
            ha.on_mqtt("ha/light.kitchen/set", r##"{"label": "Kitchen", "kind": "light", "on": true, "color": "#FFCC66", "value": "on"}"##, h)
        });
        let g = |n: &str| s.get(&format!("{ID}:light.kitchen.{n}")).cloned();
        assert_eq!(g("label"), Some(json!("Kitchen")));
        assert_eq!(g("on"), Some(json!(true)));
        assert_eq!(g("color"), Some(json!("#FFCC66")));
        assert_eq!(g("icon"), Some(json!("com.deckling.ha:light")), "kind picks an icon of the plugin's set");
        assert_eq!(ha.options("ids").unwrap(), vec![json!({"label": "Kitchen (light.kitchen)", "value": "light.kitchen"})]);
    }

    #[test]
    fn old_remote_topic_still_works() {
        let (_, s, _) = run(|ha, h| {
            ha.on_mqtt("remote/2/set", r##"{"label": "Desk", "icon": "light", "bg": "#ff0000", "icon_state": {"on": false, "color": "#00FF00"}}"##, h)
        });
        let g = |n: &str| s.get(&format!("{ID}:remote-2.{n}")).cloned();
        assert_eq!(g("background"), Some(json!("#ff0000")), "bg is the key background");
        assert_eq!(g("on"), Some(json!(false)));
        assert_eq!(g("color"), Some(json!("#00FF00")), "icon_state colour is the entity colour");
        assert_eq!(g("icon"), Some(json!("com.deckling.ha:light")));
        // an emoji stays as it is, a Night Signal name gets its namespace
        assert_eq!(icon_for(Some("💡"), None), "💡");
        assert_eq!(icon_for(Some("ha-garage"), None), "com.deckling.core:ha-garage");
    }

    #[test]
    fn clearing_removes_signals_and_id() {
        let (ha, s, _) = run(|ha, h| {
            ha.on_mqtt("ha/x/set", r#"{"label": "X", "on": true}"#, h);
            ha.on_mqtt("ha/x/set", "", h);
        });
        assert!(s.all().next().is_none());
        assert!(ha.options("ids").unwrap().is_empty());
    }

    #[test]
    fn press_goes_back_to_home_assistant() {
        let p = |v: Value| v.as_object().cloned().unwrap();
        let (_, _, out) = run(|ha, h| {
            ha.on_action("press", &p(json!({"id": "light.kitchen"})), h).unwrap().unwrap();
            ha.on_action("press", &p(json!({"id": "remote-3", "event": "long_press"})), h).unwrap().unwrap();
            assert!(ha.on_action("press", &p(json!({"id": "{id}"})), h).unwrap().is_err());
        });
        assert_eq!(out[0], ("d200/ha/light.kitchen/action".into(), "press".into(), false));
        assert_eq!(out[1], ("d200/remote/3/action".into(), "long_press".into(), false));
    }

    #[test]
    fn printer_signals() {
        let (_, s, _) = run(|ha, h| {
            ha.on_mqtt("printer", r##"{"state": "printing", "percent": 62, "spools": [{"color": "#69E0C1", "progress": 80}]}"##, h)
        });
        assert_eq!(s.get(&format!("{ID}:printer.progress")), Some(&json!(62.0)));
        assert_eq!(s.get(&format!("{ID}:printer.slot1_color")), Some(&json!("#69E0C1")));
    }
}
