//! MQTT client and Home Assistant discovery.
//!
//! Topics, with `<b>` = the configured base topic (default "d200"):
//!   <b>/availability        out, retained  "online <host>" / "offline"
//!   <b>/notify              in   text, or JSON {text, title, icon, color, style, duration}
//!   <b>/remote/<k>/set      in   read by com.deckling.ha (see ha.rs), like <b>/ha/<id>/set
//!   <b>/remote/<k>/action   out  press | long_press | down | up (remote page keys 0-12)
//!   <b>/remote/show         in   anything shows the remote page, "off" leaves it
//!   <b>/key                 out  JSON {profile, page, key, event} for every key event
//!   <b>/profile[/set]       out retained / in   current profile name
//!   <b>/page                out, retained  JSON {profile, page, remote}
//!   <b>/brightness[/set]    out retained / in   0-255
//!
//! The state that matters (remote keys) lives on the broker as retained
//! messages, so whichever computer the D200 is plugged into picks it up.

use crate::config::{KeyEvent, MqttCfg};
use rumqttc::{Client, Event, MqttOptions, Packet, QoS, Transport};
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub enum MqttMsg {
    Connected,
    Disconnected(String),
    Message { topic: String, payload: Vec<u8> },
}

pub struct Mqtt {
    client: Client,
    pub base: String,
    stop: Arc<AtomicBool>,
}

pub fn host_name() -> String {
    sysinfo::System::host_name().unwrap_or_else(|| "computer".into())
}

impl Mqtt {
    /// The deck's shared connection (with the availability last will).
    pub fn start(cfg: &MqttCfg, on: impl Fn(MqttMsg) + Send + 'static) -> Mqtt {
        Mqtt::start_as(cfg, "", true, on)
    }

    /// A connection with a client id suffix (a plugin's own broker); `will`: publish
    /// `<base>/availability` "offline" as the last will.
    pub fn start_as(cfg: &MqttCfg, suffix: &str, will: bool, on: impl Fn(MqttMsg) + Send + 'static) -> Mqtt {
        let host: String = host_name().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        let suffix: String = suffix.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        let id = if suffix.is_empty() { format!("d200deck-{host}") } else { format!("d200deck-{host}-{suffix}") };
        let mut opts = MqttOptions::new(id, cfg.host.trim(), cfg.port);
        opts.set_keep_alive(Duration::from_secs(20));
        opts.set_max_packet_size(256 * 1024, 256 * 1024);
        if !cfg.username.is_empty() {
            opts.set_credentials(cfg.username.clone(), cfg.password.clone());
        }
        if cfg.tls {
            opts.set_transport(Transport::tls_with_default_config());
        }
        let base = cfg.base_topic.trim().trim_end_matches('/').to_string();
        let base = if base.is_empty() { "d200".into() } else { base };
        if will {
            opts.set_last_will(rumqttc::LastWill::new(format!("{base}/availability"), "offline", QoS::AtLeastOnce, true));
        }
        let (client, mut conn) = Client::new(opts, 128);
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        std::thread::spawn(move || {
            for ev in conn.iter() {
                if stop2.load(Ordering::SeqCst) {
                    break;
                }
                match ev {
                    Ok(Event::Incoming(Packet::ConnAck(_))) => on(MqttMsg::Connected),
                    Ok(Event::Incoming(Packet::Publish(p))) => {
                        on(MqttMsg::Message { topic: p.topic.to_string(), payload: p.payload.to_vec() })
                    }
                    Ok(_) => {}
                    Err(e) => {
                        on(MqttMsg::Disconnected(e.to_string()));
                        for _ in 0..30 {
                            if stop2.load(Ordering::SeqCst) {
                                return;
                            }
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    }
                }
            }
        });
        Mqtt { client, base, stop }
    }

    pub fn topic(&self, rest: &str) -> String {
        format!("{}/{rest}", self.base)
    }

    pub fn publish(&self, topic: &str, payload: impl Into<Vec<u8>>, retain: bool) {
        let _ = self.client.try_publish(topic, QoS::AtLeastOnce, retain, payload.into());
    }

    pub fn subscribe_all(&self) {
        for t in ["notify", "remote/show", "profile/set", "brightness/set", "availability", "page/set", "screensaver/set", "display/set",
                  "follow/set", "notify/clear", "pomodoro/set", "press", "action"] {
            let _ = self.client.try_subscribe(self.topic(t), QoS::AtLeastOnce);
        }
        let _ = self.client.try_subscribe("homeassistant/status", QoS::AtLeastOnce);
    }

    /// Subscribe to `<base>/<rest>` (for a plugin's topics).
    pub fn subscribe(&self, rest: &str) {
        let _ = self.client.try_subscribe(self.topic(rest), QoS::AtLeastOnce);
    }

    pub fn stop(self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.client.try_disconnect();
    }

    /// Retained Home Assistant discovery configs. An empty payload removes an entity
    /// (the old remote button and device triggers, Teams entities while Teams is off).
    pub fn discovery(&self, profiles: &[String], pages: &[String], teams: bool) -> Vec<(String, String)> {
        discovery(&self.base, profiles, pages, teams)
    }
}

/// Retained Home Assistant discovery configs for the base topic `b`.
pub fn discovery(b: &str, profiles: &[String], pages: &[String], teams: bool) -> Vec<(String, String)> {
    let id = format!("{}_deck", b.replace('/', "_"));
    let device = json!({
        "identifiers": [id],
        "name": "Ulanzi D200",
        "manufacturer": "Ulanzi",
        "model": "D200 (d200fw)",
    });
    let avail = json!([{ "topic": format!("{b}/availability"), "value_template": "{{ value.split(' ')[0] }}" }]);
    let state = format!("{b}/state");
    let mut out = vec![];
    // Gone: 26 device triggers for the remote page (the key event entity has every key),
    // and the remote button (now a switch).
    for k in 0..13 {
        for ev in [KeyEvent::Press, KeyEvent::Long] {
            out.push((format!("homeassistant/device_automation/{id}/remote_{k}_{}/config", ev.name()), String::new()));
        }
    }
    out.push((format!("homeassistant/button/{id}/remote/config"), String::new()));

    let mut add = |component: &str, key: &str, mut cfg: serde_json::Value| {
        let o = cfg.as_object_mut().unwrap();
        o.insert("unique_id".into(), json!(format!("{id}_{key}")));
        o.insert("device".into(), device.clone());
        // availability: the computer's; `null` means always available (the computer sensors)
        match o.get("availability") {
            None => {
                o.insert("availability".into(), avail.clone());
            }
            Some(v) if v.is_null() => {
                o.remove("availability");
            }
            _ => {}
        }
        out.push((format!("homeassistant/{component}/{id}/{key}/config"), cfg.to_string()));
    };
    let on_off = |v: &str| format!("{{{{ 'ON' if value_json.{v} else 'OFF' }}}}");

    // ---- controls
    add("notify", "notify", json!({ "name": "Notification", "command_topic": format!("{b}/notify") }));
    add("button", "notify_clear", json!({ "name": "Clear notification", "icon": "mdi:bell-off",
        "command_topic": format!("{b}/notify/clear"), "payload_press": "clear" }));
    add("select", "profile", json!({ "name": "Profile", "icon": "mdi:view-grid",
        "command_topic": format!("{b}/profile/set"), "state_topic": format!("{b}/profile"), "options": profiles }));
    add("select", "page", json!({ "name": "Page", "icon": "mdi:book-open-page-variant",
        "command_topic": format!("{b}/page/set"), "state_topic": state, "value_template": "{{ value_json.page }}", "options": pages }));
    add("number", "brightness", json!({ "name": "Brightness", "icon": "mdi:brightness-6",
        "command_topic": format!("{b}/brightness/set"), "state_topic": format!("{b}/brightness"), "min": 0, "max": 255, "mode": "slider" }));
    add("switch", "remote_page", json!({ "name": "Remote page", "icon": "mdi:remote",
        "command_topic": format!("{b}/remote/show"), "payload_on": "on", "payload_off": "off",
        "state_topic": state, "value_template": on_off("remote"), "state_on": "ON", "state_off": "OFF" }));
    add("switch", "screensaver", json!({ "name": "Screensaver", "icon": "mdi:weather-night",
        "command_topic": format!("{b}/screensaver/set"), "payload_on": "on", "payload_off": "off",
        "state_topic": state, "value_template": on_off("screensaver"), "state_on": "ON", "state_off": "OFF" }));
    add("switch", "display", json!({ "name": "Display", "icon": "mdi:monitor",
        "command_topic": format!("{b}/display/set"), "payload_on": "on", "payload_off": "off",
        "state_topic": state, "value_template": on_off("display"), "state_on": "ON", "state_off": "OFF" }));
    add("switch", "follow", json!({ "name": "Follow program in front", "icon": "mdi:application-cog", "entity_category": "config",
        "command_topic": format!("{b}/follow/set"), "payload_on": "on", "payload_off": "off",
        "state_topic": state, "value_template": on_off("follow"), "state_on": "ON", "state_off": "OFF" }));
    add("button", "pomodoro", json!({ "name": "Pomodoro start or pause", "icon": "mdi:timer-play",
        "command_topic": format!("{b}/pomodoro/set"), "payload_press": "press" }));
    add("button", "pomodoro_reset", json!({ "name": "Pomodoro reset", "icon": "mdi:timer-refresh",
        "command_topic": format!("{b}/pomodoro/set"), "payload_press": "reset" }));

    // ---- what the deck knows
    add("binary_sensor", "connected", json!({ "name": "Computer online", "device_class": "connectivity", "entity_category": "diagnostic",
        "state_topic": format!("{b}/availability"), "value_template": "{{ 'ON' if value.startswith('online') else 'OFF' }}",
        "availability": serde_json::Value::Null }));
    add("sensor", "host", json!({ "name": "Computer", "icon": "mdi:monitor", "entity_category": "diagnostic",
        "state_topic": format!("{b}/availability"),
        "value_template": "{{ value.split(' ', 1)[1] if value.startswith('online ') else 'none' }}", "availability": serde_json::Value::Null }));
    add("binary_sensor", "usb", json!({ "name": "D200 plugged in", "device_class": "plug", "entity_category": "diagnostic",
        "state_topic": state, "value_template": on_off("usb") }));
    add("sensor", "app", json!({ "name": "Program in front", "icon": "mdi:application",
        "state_topic": state, "value_template": "{{ value_json.app or 'none' }}" }));
    add("sensor", "now_playing", json!({ "name": "Now playing", "icon": "mdi:music",
        "state_topic": state,
        "value_template": "{{ (value_json.media.title ~ ' – ' ~ value_json.media.artist) if value_json.media else 'idle' }}",
        "json_attributes_topic": state, "json_attributes_template": "{{ (value_json.media or {}) | tojson }}" }));
    add("sensor", "pomodoro_phase", json!({ "name": "Pomodoro", "icon": "mdi:timer-outline",
        "state_topic": state, "value_template": "{{ value_json.pomodoro.phase or 'idle' }}",
        "json_attributes_topic": state, "json_attributes_template": "{{ value_json.pomodoro | tojson }}" }));
    add("update", "app", json!({ "name": "D200 Deck", "entity_category": "diagnostic",
        "state_topic": state,
        "value_template": "{{ {'installed_version': value_json.version, 'latest_version': value_json.latest, 'title': 'D200 Deck', \
'release_url': 'https://github.com/ricktap/ulanzi-d200-cfw/releases/tag/v' ~ value_json.latest} | tojson }}" }));

    // ---- Teams, while the plugin is on
    let presence = format!("homeassistant/sensor/{id}/teams_presence/config");
    let meeting = format!("homeassistant/binary_sensor/{id}/teams_meeting/config");
    if teams {
        add("sensor", "teams_presence", json!({ "name": "Teams presence", "icon": "mdi:account-circle",
            "state_topic": state, "value_template": "{{ value_json.teams.presence if value_json.teams and value_json.teams.presence else 'unknown' }}",
            "json_attributes_topic": state, "json_attributes_template": "{{ (value_json.teams or {}) | tojson }}" }));
        add("binary_sensor", "teams_meeting", json!({ "name": "In a meeting", "icon": "mdi:video",
            "state_topic": state, "value_template": "{{ 'ON' if value_json.teams and value_json.teams.in_meeting else 'OFF' }}" }));
    } else {
        out.push((presence, String::new()));
        out.push((meeting, String::new()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::discovery;
    use serde_json::Value;

    #[test]
    fn discovery_entities() {
        let out = discovery("d200", &["Default".into()], &["main".into(), "Media".into()], false);
        let get = |t: &str| out.iter().find(|(topic, _)| topic == t).map(|(_, p)| p.clone()).unwrap_or_else(|| panic!("{t}"));
        // the old remote-page device triggers and the remote button are removed
        assert_eq!(get("homeassistant/device_automation/d200_deck/remote_0_press/config"), "");
        assert_eq!(get("homeassistant/button/d200_deck/remote/config"), "");
        assert_eq!(get("homeassistant/sensor/d200_deck/teams_presence/config"), "");
        let page: Value = serde_json::from_str(&get("homeassistant/select/d200_deck/page/config")).unwrap();
        assert_eq!(page["options"], serde_json::json!(["main", "Media"]));
        assert_eq!(page["command_topic"], "d200/page/set");
        assert_eq!(page["device"]["identifiers"][0], "d200_deck");
        for t in ["switch/d200_deck/remote_page", "switch/d200_deck/screensaver", "switch/d200_deck/display", "switch/d200_deck/follow",
                  "button/d200_deck/notify_clear", "button/d200_deck/pomodoro", "binary_sensor/d200_deck/usb", "sensor/d200_deck/app",
                  "sensor/d200_deck/now_playing", "sensor/d200_deck/pomodoro_phase", "update/d200_deck/app"] {
            let v: Value = serde_json::from_str(&get(&format!("homeassistant/{t}/config"))).unwrap();
            assert!(v["availability"].is_array(), "{t}");
        }
        // the computer sensors stay available when no computer has the deck
        let c: Value = serde_json::from_str(&get("homeassistant/binary_sensor/d200_deck/connected/config")).unwrap();
        assert!(c.get("availability").is_none());
        assert_eq!(c["name"], "Computer online");
        let teams = discovery("d200", &[], &[], true);
        assert!(teams.iter().any(|(t, p)| t.ends_with("teams_meeting/config") && !p.is_empty()));
    }

    /// tests/ha/discovery.json: what the deck announces, for the Home Assistant tests
    /// (`UPDATE_FIXTURES=1 cargo test --lib` writes it).
    #[test]
    fn discovery_fixture_is_current() {
        let out = discovery("d200", &["Default".into(), "Work".into()], &["main".into(), "Media".into()], true);
        let v: serde_json::Map<String, Value> = out.into_iter().map(|(t, p)| (t, Value::String(p))).collect();
        let text = serde_json::to_string_pretty(&v).unwrap() + "\n";
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/ha/discovery.json");
        if std::env::var_os("UPDATE_FIXTURES").is_some() {
            std::fs::write(path, &text).unwrap();
        }
        assert_eq!(std::fs::read_to_string(path).unwrap_or_default(), text, "run UPDATE_FIXTURES=1 cargo test --lib");
    }
}
