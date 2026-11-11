//! Built-in plugins: the interface between the engine and a plugin that runs in the
//! engine's thread (docs/deckling/architecture.md, section 5.3).
//!
//! A plugin publishes signals in its own namespace, takes the actions it registered,
//! and fills `select` fields from a `source`. It never draws, never sets a colour on a
//! key and never sees key presses. MQTT is part of the app: a plugin names the topics
//! it reads (relative to the base topic) and may publish through the host.

use crate::bind::Signals;
use crate::config::Config;
use serde_json::{Map, Value};
use std::any::Any;
use std::path::PathBuf;

/// A message a plugin's background work sends back to it (delivered to `on_message`).
pub type PluginMsg = Box<dyn Any + Send>;

/// Hands results from a plugin's threads back to the engine thread.
#[derive(Clone)]
pub struct Sender {
    id: &'static str,
    tx: std::sync::mpsc::Sender<crate::engine::Msg>,
}

impl Sender {
    pub fn new(id: &'static str, tx: std::sync::mpsc::Sender<crate::engine::Msg>) -> Sender {
        Sender { id, tx }
    }
    pub fn send(&self, msg: impl Any + Send) {
        let _ = self.tx.send(crate::engine::Msg::Plugin(self.id, Box::new(msg)));
    }
}

/// What a plugin may touch while it handles one call.
pub struct Host<'a> {
    id: &'static str,
    signals: &'a mut Signals,
    /// The configured MQTT base topic ("d200").
    pub base: String,
    /// (absolute topic, payload, retain) to publish when the call returns.
    pub mqtt_out: Vec<(String, String, bool)>,
    /// The app's data folder (next to deck.json), for tokens and the like.
    pub dir: PathBuf,
    sender: Option<Sender>,
}

impl<'a> Host<'a> {
    pub fn new(id: &'static str, signals: &'a mut Signals, base: String) -> Host<'a> {
        Host { id, signals, base, mqtt_out: vec![], dir: PathBuf::new(), sender: None }
    }

    pub fn with_engine(mut self, dir: PathBuf, sender: Sender) -> Host<'a> {
        self.dir = dir;
        self.sender = Some(sender);
        self
    }

    /// For background threads: what they send arrives in `on_message`.
    pub fn sender(&self) -> Option<Sender> {
        self.sender.clone()
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.signals.get(&self.full(name))
    }

    fn full(&self, name: &str) -> String {
        format!("{}:{name}", self.id)
    }

    /// Publish `<plugin id>:<name>`. Returns true when the value changed.
    pub fn publish(&mut self, name: &str, value: Value) -> bool {
        let n = self.full(name);
        self.signals.publish(&n, value)
    }

    pub fn clear(&mut self, name: &str) -> bool {
        let n = self.full(name);
        self.signals.clear(&n)
    }

    /// Clear every signal of this plugin that starts with `prefix`.
    pub fn clear_prefix(&mut self, prefix: &str) {
        let p = self.full(prefix);
        let names: Vec<String> = self.signals.all().map(|(n, _)| n.clone()).filter(|n| n.starts_with(&p)).collect();
        for n in names {
            self.signals.clear(&n);
        }
    }

    /// Publish to `<base>/<topic>` over the app's MQTT connection.
    pub fn mqtt(&mut self, topic: &str, payload: impl Into<String>, retain: bool) {
        let t = format!("{}/{topic}", self.base);
        self.mqtt_out.push((t, payload.into(), retain));
    }

    /// Publish to an absolute topic (the `mqtt` action).
    pub fn mqtt_abs(&mut self, topic: &str, payload: impl Into<String>, retain: bool) {
        self.mqtt_out.push((topic.to_string(), payload.into(), retain));
    }
}

pub trait Plugin: Send {
    fn id(&self) -> &'static str;

    /// MQTT topics this plugin reads, relative to the base topic (`+` and `#` allowed).
    fn mqtt_topics(&self) -> &'static [&'static str] {
        &[]
    }

    /// A message on one of `mqtt_topics` (topic relative to the base topic).
    fn on_mqtt(&mut self, _topic: &str, _payload: &str, _host: &mut Host) {}

    /// Run action `<plugin id>:<name>`. `None`: not one of this plugin's actions.
    fn on_action(&mut self, _name: &str, _params: &Map<String, Value>, _host: &mut Host) -> Option<Result<(), String>> {
        None
    }

    /// Values for a `select` with `{"source": "<plugin id>:<name>"}`.
    fn options(&self, _name: &str) -> Option<Vec<Value>> {
        None
    }

    /// At start and after every config change: start or stop what the deck needs.
    fn configure(&mut self, _cfg: &Config, _host: &mut Host) {}

    /// Once a second.
    fn tick(&mut self, _ticks: u64, _cfg: &Config, _host: &mut Host) {}

    /// A result from this plugin's own background work (see `Host::sender`).
    fn on_message(&mut self, _msg: PluginMsg, _host: &mut Host) {}

    /// Connection state for the UI (merged into the `integrations` event).
    fn status(&self) -> Value {
        Value::Null
    }
}

/// MQTT topic filter match (`+` one level, `#` the rest).
pub fn topic_matches(filter: &str, topic: &str) -> bool {
    let mut f = filter.split('/');
    let mut t = topic.split('/');
    loop {
        match (f.next(), t.next()) {
            (Some("#"), _) => return true,
            (Some("+"), Some(_)) => {}
            (Some(a), Some(b)) if a == b => {}
            (None, None) => return true,
            _ => return false,
        }
    }
}

/// Replace `{key}` in string parameters with the key's setting `key`. Placeholders
/// that name no setting stay as they are (a shell `${HOME}` is not touched).
pub fn fill_params(params: &Map<String, Value>, config: &Map<String, Value>) -> Map<String, Value> {
    params
        .iter()
        .map(|(k, v)| {
            let v = match v.as_str() {
                Some(s) if s.contains('{') => {
                    let mut out = s.to_string();
                    for (ck, cv) in config {
                        out = out.replace(&format!("{{{ck}}}"), &crate::bind::text(cv));
                    }
                    Value::String(out)
                }
                _ => v.clone(),
            };
            (k.clone(), v)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn topics() {
        assert!(topic_matches("ha/+/set", "ha/kitchen/set"));
        assert!(!topic_matches("ha/+/set", "ha/kitchen/light/set"));
        assert!(topic_matches("ha/#", "ha/kitchen/light/set"));
        assert!(!topic_matches("printer", "printer/x"));
    }

    #[test]
    fn placeholders_only_from_settings() {
        let p = json!({"id": "{id}", "cmd": "echo ${HOME} {id}", "n": 3}).as_object().cloned().unwrap();
        let c = json!({"id": "light.kitchen"}).as_object().cloned().unwrap();
        let out = fill_params(&p, &c);
        assert_eq!(out["id"], "light.kitchen");
        assert_eq!(out["cmd"], "echo ${HOME} light.kitchen");
        assert_eq!(out["n"], 3);
    }

    #[test]
    fn host_publishes_in_own_namespace() {
        let mut s = Signals::default();
        let mut h = Host::new("com.deckling.ha", &mut s, "d200".into());
        h.publish("x.on", json!(true));
        h.mqtt("ha/x/action", "press", false);
        assert_eq!(h.mqtt_out[0].0, "d200/ha/x/action");
        assert_eq!(s.get("com.deckling.ha:x.on"), Some(&json!(true)));
    }
}
