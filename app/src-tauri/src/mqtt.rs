//! MQTT client and Home Assistant discovery.
//!
//! Topics, with `<b>` = the configured base topic (default "d200"):
//!   <b>/availability        out, retained  "online <host>" / "offline"
//!   <b>/notify              in   text, or JSON {text, title, icon, color, style, duration}
//!   <b>/remote/<k>/set      in   retained JSON {label, icon, bg, fg, value}; empty clears
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
    pub fn start(cfg: &MqttCfg, on: impl Fn(MqttMsg) + Send + 'static) -> Mqtt {
        let host: String = host_name().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        let mut opts = MqttOptions::new(format!("d200deck-{host}"), cfg.host.trim(), cfg.port);
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
        opts.set_last_will(rumqttc::LastWill::new(format!("{base}/availability"), "offline", QoS::AtLeastOnce, true));
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
        for t in ["notify", "remote/+/set", "remote/show", "profile/set", "brightness/set", "availability", "printer"] {
            let _ = self.client.try_subscribe(self.topic(t), QoS::AtLeastOnce);
        }
        let _ = self.client.try_subscribe("homeassistant/status", QoS::AtLeastOnce);
    }

    pub fn stop(self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.client.try_disconnect();
    }

    /// Retained Home Assistant discovery configs.
    pub fn discovery(&self, profiles: &[String]) -> Vec<(String, String)> {
        let b = &self.base;
        let id = format!("{}_deck", b.replace('/', "_"));
        let device = json!({
            "identifiers": [id],
            "name": "Ulanzi D200",
            "manufacturer": "Ulanzi",
            "model": "D200 (d200fw)",
        });
        let avail = json!([{ "topic": format!("{b}/availability"), "value_template": "{{ value.split(' ')[0] }}" }]);
        let mut out = vec![];
        for k in 0..13 {
            for (ev, ty) in [(KeyEvent::Press, "button_short_press"), (KeyEvent::Long, "button_long_press")] {
                out.push((
                    format!("homeassistant/device_automation/{id}/remote_{k}_{}/config", ev.name()),
                    json!({
                        "automation_type": "trigger",
                        "topic": format!("{b}/remote/{k}/action"),
                        "payload": ev.name(),
                        "type": ty,
                        "subtype": format!("button_{}", k + 1),
                        "device": device,
                    })
                    .to_string(),
                ));
            }
        }
        out.push((
            format!("homeassistant/notify/{id}/notify/config"),
            json!({ "name": "Notification", "unique_id": format!("{id}_notify"), "command_topic": format!("{b}/notify"),
                    "availability": avail, "device": device }).to_string(),
        ));
        out.push((
            format!("homeassistant/select/{id}/profile/config"),
            json!({ "name": "Profile", "unique_id": format!("{id}_profile"), "icon": "mdi:view-grid",
                    "command_topic": format!("{b}/profile/set"), "state_topic": format!("{b}/profile"),
                    "options": profiles, "availability": avail, "device": device }).to_string(),
        ));
        out.push((
            format!("homeassistant/number/{id}/brightness/config"),
            json!({ "name": "Brightness", "unique_id": format!("{id}_brightness"), "icon": "mdi:brightness-6",
                    "command_topic": format!("{b}/brightness/set"), "state_topic": format!("{b}/brightness"),
                    "min": 0, "max": 255, "mode": "slider", "availability": avail, "device": device }).to_string(),
        ));
        out.push((
            format!("homeassistant/button/{id}/remote/config"),
            json!({ "name": "Show remote page", "unique_id": format!("{id}_remote"), "icon": "mdi:remote",
                    "command_topic": format!("{b}/remote/show"), "availability": avail, "device": device }).to_string(),
        ));
        out.push((
            format!("homeassistant/binary_sensor/{id}/connected/config"),
            json!({ "name": "Connected", "unique_id": format!("{id}_connected"), "device_class": "connectivity",
                    "state_topic": format!("{b}/availability"),
                    "value_template": "{{ 'ON' if value.startswith('online') else 'OFF' }}", "device": device }).to_string(),
        ));
        out.push((
            format!("homeassistant/sensor/{id}/host/config"),
            json!({ "name": "Computer", "unique_id": format!("{id}_host"), "icon": "mdi:monitor",
                    "state_topic": format!("{b}/availability"),
                    "value_template": "{{ value.split(' ', 1)[1] if value.startswith('online ') else 'none' }}", "device": device }).to_string(),
        ));
        out
    }
}
