//! com.deckling.web: an HTTP API on the local network, so scripts and other tools can
//! drive the deck without MQTT.
//!
//! Off until `serve` is switched on in the plugin's settings, and only with a token: every
//! request carries `Authorization: Bearer <token>` (or `?token=`). The server runs in its
//! own thread; each request goes to the plugin in the engine's thread, which answers it.
//!
//! | Method and path | Does |
//! |---|---|
//! | `GET /api` | what this server offers |
//! | `GET /api/signals`, `GET /api/signals/<name>` | every signal, or one |
//! | `PUT /api/signals/<name>` | sets `com.deckling.web:<name>` (body: any JSON value, or `{"value": ...}`) |
//! | `GET /api/keys`, `PUT /api/keys/<id>`, `DELETE /api/keys/<id>` | keys by id, as `ha/<id>/set` (label, kind, icon, value, on, color, progress, background) |
//! | `POST /api/notify` | a notification: text or JSON like MQTT `notify` |
//! | `POST /api/actions/<action>` | runs an action with `{"params": {...}}` |
//! | `POST /api/keys/<0-13>/press` | presses a key of the page on the device (`{"event": "long"}` for a long press) |
//!
//! A press on a key with the app `com.deckling.web:key` is posted to the `webhook` URL as
//! `{"id": ..., "event": "press" | "long_press"}`.

use crate::config::{Config, KeyEvent};
use crate::plugins::{Host, HostRequest, Plugin, PluginMsg};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

pub const ID: &str = "com.deckling.web";
const MAX_BODY: usize = 64 * 1024;
/// Actions that start programs or type on the computer: refused unless `allow_programs`.
pub const PROGRAM_ACTIONS: [&str; 3] = [
    "com.deckling.core:command",
    "com.deckling.core:open",
    "com.deckling.core:keys",
];

/// One HTTP request, handed to the plugin in the engine's thread.
pub struct Request {
    pub method: String,
    pub path: String,
    pub body: Value,
    pub reply: mpsc::Sender<(u16, Value)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Settings {
    serve: bool,
    port: u16,
    token: String,
    allow_programs: bool,
    webhook: String,
}

impl Settings {
    fn from(cfg: &Config) -> Settings {
        let c = cfg.plugins.get(ID).map(|p| &p.config);
        let get = |k: &str| c.and_then(|c| c.get(k));
        Settings {
            serve: get("serve").and_then(Value::as_bool).unwrap_or(false),
            port: get("port")
                .and_then(Value::as_u64)
                .and_then(|p| u16::try_from(p).ok())
                .filter(|p| *p > 0)
                .unwrap_or(8765),
            token: get("token")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string(),
            allow_programs: get("allow_programs")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            webhook: get("webhook")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string(),
        }
    }
}

struct Server {
    stop: Arc<AtomicBool>,
    port: u16,
    token: String,
    /// The mDNS announcement (`_d200deck._tcp`), so Home Assistant can discover the deck.
    mdns: Option<(mdns_sd::ServiceDaemon, String)>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some((d, name)) = self.mdns.take() {
            let _ = d.unregister(&name);
            let _ = d.shutdown();
        }
    }
}

/// The mDNS service type the deck announces while the server runs.
pub const MDNS_TYPE: &str = "_d200deck._tcp.local.";

/// Announce the server on the local network; None when mDNS is not available.
fn announce(port: u16) -> Option<(mdns_sd::ServiceDaemon, String)> {
    let host = crate::mqtt::host_name();
    let label: String = host.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
    let daemon = mdns_sd::ServiceDaemon::new().ok()?;
    let props = [("id", label.as_str()), ("version", env!("CARGO_PKG_VERSION")), ("path", "/api")];
    let info = mdns_sd::ServiceInfo::new(MDNS_TYPE, &format!("D200 Deck on {host}"), &format!("{label}-d200deck.local."), "", port, &props[..])
        .ok()?
        .enable_addr_auto();
    let name = info.get_fullname().to_string();
    daemon.register(info).ok()?;
    Some((daemon, name))
}

#[derive(Default)]
pub struct Web {
    settings: Settings,
    server: Option<Server>,
    error: Option<String>,
    /// id -> label, for the id picker.
    ids: BTreeMap<String, String>,
    /// The deck's shared MQTT settings, for GET /api/mqtt.
    core_mqtt: crate::config::MqttCfg,
    /// Webhook URLs registered over the API (Home Assistant), until they expire, and
    /// whether they also want every key event.
    subs: BTreeMap<String, Sub>,
}

#[derive(Debug, Clone, Copy)]
struct Sub {
    until: std::time::Instant,
    keys: bool,
    /// Also every change of `deck.state`.
    state: bool,
}

/// At most this many subscriptions; each lives up to an hour unless renewed.
const MAX_SUBS: usize = 16;
const MAX_TTL: u64 = 3600;

impl Web {
    fn start(&mut self, host: &Host) {
        let s = &self.settings;
        let want = s.serve && !s.token.is_empty();
        let same = self
            .server
            .as_ref()
            .is_some_and(|sv| sv.port == s.port && sv.token == s.token);
        if !want {
            self.server = None;
            self.error = s.serve.then(|| "Set a token first".to_string());
            return;
        }
        if same {
            return;
        }
        self.server = None; // stops the old one
        let Some(sender) = host.sender() else { return };
        let listener = match TcpListener::bind(("0.0.0.0", s.port)) {
            Ok(l) => l,
            Err(e) => {
                self.error = Some(format!("port {}: {e}", s.port));
                return;
            }
        };
        let _ = listener.set_nonblocking(true);
        let stop = Arc::new(AtomicBool::new(false));
        let (st, token) = (stop.clone(), s.token.clone());
        std::thread::spawn(move || {
            while !st.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let (sender, token) = (sender.clone(), token.clone());
                        std::thread::spawn(move || {
                            let _ = serve(stream, &token, &sender);
                        });
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(100)),
                }
            }
        });
        self.error = None;
        let mdns = if std::env::var_os("D200_NO_MDNS").is_some() { None } else { announce(s.port) };
        self.server = Some(Server {
            stop,
            port: s.port,
            token: s.token.clone(),
            mdns,
        });
    }

    fn handle(&mut self, method: &str, path: &str, body: Value, host: &mut Host) -> (u16, Value) {
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        match (method, parts.as_slice()) {
            ("GET", ["api"]) => (
                200,
                json!({
                    "name": "D200 Deck", "version": env!("CARGO_PKG_VERSION"),
                    "openapi": "/api/openapi.json",
                    "endpoints": ["GET /api/state", "GET /api/signals", "GET|PUT /api/signals/<name>", "GET /api/keys", "PUT|DELETE /api/keys/<id>",
                                  "POST /api/notify", "POST /api/actions/<action>", "POST /api/keys/<0-13>/press",
                                  "GET|POST|DELETE /api/subscriptions", "GET|PUT /api/mqtt"],
                }),
            ),
            ("GET", ["api", "mqtt"]) => {
                let m = &self.core_mqtt;
                (200, json!({ "enabled": m.enabled, "host": m.host, "port": m.port, "username": m.username,
                              "has_password": !m.password.is_empty(), "tls": m.tls, "base_topic": m.base_topic, "discovery": m.discovery }))
            }
            ("PUT", ["api", "mqtt"]) => {
                let host_name = body.get("host").and_then(Value::as_str).unwrap_or("").trim().to_string();
                if host_name.is_empty() || host_name.len() > 255 {
                    return (400, json!({ "error": "host is required" }));
                }
                let mut m = self.core_mqtt.clone();
                m.enabled = body.get("enabled").and_then(Value::as_bool).unwrap_or(true);
                m.host = host_name;
                if let Some(p) = body.get("port").and_then(Value::as_u64).and_then(|p| u16::try_from(p).ok()) {
                    m.port = p;
                }
                if let Some(u) = body.get("username").and_then(Value::as_str) {
                    m.username = u.to_string();
                }
                if let Some(p) = body.get("password").and_then(Value::as_str) {
                    m.password = p.to_string();
                }
                if let Some(t) = body.get("tls").and_then(Value::as_bool) {
                    m.tls = t;
                }
                if let Some(b) = body.get("base_topic").and_then(Value::as_str).map(str::trim).filter(|b| !b.is_empty() && !b.contains(['+', '#'])) {
                    m.base_topic = b.to_string();
                }
                self.core_mqtt = m.clone();
                host.request(HostRequest::SetMqtt(m));
                (200, json!({ "saved": true }))
            }
            ("GET", ["api", "subscriptions"]) => {
                let now = std::time::Instant::now();
                self.subs.retain(|_, s| s.until > now);
                let list: Vec<Value> = self.subs.iter().map(|(u, s)| json!({ "url": u, "expires_in": s.until.duration_since(now).as_secs(), "keys": s.keys, "state": s.state })).collect();
                (200, json!(list))
            }
            ("POST", ["api", "subscriptions"]) => {
                let url = body.get("url").and_then(Value::as_str).unwrap_or("").trim().to_string();
                if !(url.starts_with("http://") || url.starts_with("https://")) || url.len() > 2048 {
                    return (400, json!({ "error": "url must be an http:// or https:// URL" }));
                }
                let now = std::time::Instant::now();
                self.subs.retain(|_, s| s.until > now);
                if !self.subs.contains_key(&url) && self.subs.len() >= MAX_SUBS {
                    return (400, json!({ "error": format!("at most {MAX_SUBS} subscriptions") }));
                }
                let ttl = body.get("ttl").and_then(Value::as_u64).unwrap_or(900).clamp(10, MAX_TTL);
                // "events": ["presses"] (default) or ["presses", "keys"]: every key event too
                let wants = |w: &str| body.get("events").and_then(Value::as_array).is_some_and(|e| e.iter().any(|x| x == w));
                let (keys, state) = (wants("keys"), wants("state"));
                self.subs.insert(url.clone(), Sub { until: now + Duration::from_secs(ttl), keys, state });
                (200, json!({ "url": url, "expires_in": ttl, "keys": keys, "state": state }))
            }
            ("DELETE", ["api", "subscriptions"]) => {
                let url = body.get("url").and_then(Value::as_str).unwrap_or("");
                match self.subs.remove(url) {
                    Some(_) => (200, json!({ "removed": url })),
                    None => (404, json!({ "error": "no such subscription" })),
                }
            }
            ("GET", ["api", "state"]) => match host.all_signals().into_iter().find(|(n, _)| n == "deck.state") {
                Some((_, v)) => (200, v),
                None => (503, json!({ "error": "the deck has no state yet" })),
            },
            ("GET", ["api", "signals"]) => {
                let mut all = host.all_signals();
                all.sort_by(|a, b| a.0.cmp(&b.0));
                (200, Value::Object(all.into_iter().collect()))
            }
            ("GET", ["api", "signals", name]) => {
                match host.all_signals().into_iter().find(|(n, _)| n == name) {
                    Some((_, v)) => (200, json!({ "name": name, "value": v })),
                    None => (404, json!({ "error": format!("no signal {name}") })),
                }
            }
            ("PUT" | "POST", ["api", "signals", name]) => {
                if !valid_name(name) {
                    return (
                        400,
                        json!({ "error": "a signal name is letters, digits, _, - and ." }),
                    );
                }
                let v = match body {
                    Value::Object(ref o) if o.len() == 1 && o.contains_key("value") => {
                        o["value"].clone()
                    }
                    other => other,
                };
                host.publish(name, v);
                (200, json!({ "signal": format!("{ID}:{name}") }))
            }
            ("GET", ["api", "keys"]) => (200, json!(self.ids)),
            ("POST", ["api", "keys", k, "press"]) => match k.parse::<usize>() {
                Ok(key) if key < crate::config::NUM_KEYS => {
                    let event = match body.get("event").and_then(Value::as_str).unwrap_or("press") {
                        "long" | "long_press" => KeyEvent::Long,
                        "down" => KeyEvent::Down,
                        "up" => KeyEvent::Up,
                        "double" | "double_press" => KeyEvent::Double,
                        _ => KeyEvent::Press,
                    };
                    host.request(HostRequest::Press { key, event });
                    (200, json!({ "pressed": key }))
                }
                _ => (400, json!({ "error": "keys are 0 to 13" })),
            },
            ("PUT" | "POST", ["api", "keys", id]) => {
                if !valid_name(id) {
                    return (
                        400,
                        json!({ "error": "an id is letters, digits, _, - and ." }),
                    );
                }
                host.clear_prefix(&format!("{id}."));
                let v = if body.is_object() {
                    body
                } else {
                    json!({ "label": crate::bind::text(&body) })
                };
                let label = crate::plugins::ha::describe(id, &v, host);
                self.ids.insert(id.to_string(), label);
                (200, json!({ "key": id }))
            }
            ("DELETE", ["api", "keys", id]) => {
                host.clear_prefix(&format!("{id}."));
                match self.ids.remove(*id) {
                    Some(_) => (200, json!({ "removed": id })),
                    None => (404, json!({ "error": format!("no key {id}") })),
                }
            }
            ("POST", ["api", "notify"]) => {
                let text = match body {
                    Value::String(s) => s,
                    Value::Null => return (400, json!({ "error": "send text or a notification" })),
                    other => other.to_string(),
                };
                host.request(HostRequest::Notify(text));
                (200, json!({ "shown": true }))
            }
            ("POST", ["api", "actions", action]) => {
                if !crate::registry::current().actions.contains_key(*action) {
                    return (404, json!({ "error": format!("no action {action}") }));
                }
                if PROGRAM_ACTIONS.contains(action) && !self.settings.allow_programs {
                    return (
                        403,
                        json!({ "error": format!("{action} starts programs; allow it in the plugin's settings") }),
                    );
                }
                let params = body
                    .get("params")
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                host.request(HostRequest::Action(crate::config::ActionRef {
                    action: action.to_string(),
                    params,
                    ..Default::default()
                }));
                (200, json!({ "ran": action }))
            }
            _ => (404, json!({ "error": format!("no {method} {path}") })),
        }
    }
}

impl Plugin for Web {
    fn id(&self) -> &'static str {
        ID
    }

    fn configure(&mut self, cfg: &Config, host: &mut Host) {
        self.core_mqtt = cfg.settings.mqtt.clone();
        let s = Settings::from(cfg);
        if s != self.settings || self.server.is_none() {
            self.settings = s;
            self.start(host);
        }
    }

    fn stop(&mut self) {
        // dropping the server stops its thread and the mDNS announcement
        self.server = None;
        self.error = None;
        self.subs.clear();
    }

    fn on_message(&mut self, msg: PluginMsg, host: &mut Host) {
        if let Ok(req) = msg.downcast::<Request>() {
            let r = self.handle(&req.method, &req.path, req.body, host);
            let _ = req.reply.send(r);
        }
    }

    fn on_action(
        &mut self,
        name: &str,
        params: &Map<String, Value>,
        host: &mut Host,
    ) -> Option<Result<(), String>> {
        if name != "press" {
            return None;
        }
        let id = params
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let event = params
            .get("event")
            .and_then(Value::as_str)
            .unwrap_or("press")
            .to_string();
        host.publish(&format!("{id}.pressed"), json!(event));
        // the configured webhook and every live subscription get the press
        let now = std::time::Instant::now();
        self.subs.retain(|_, s| s.until > now);
        let mut urls: Vec<String> = self.subs.keys().cloned().collect();
        let hook = self.settings.webhook.clone();
        if (hook.starts_with("http://") || hook.starts_with("https://")) && !urls.contains(&hook) {
            urls.push(hook);
        }
        for url in urls {
            let body = json!({ "id": id, "event": event });
            std::thread::spawn(move || {
                let _ = ureq::post(&url).timeout(Duration::from_secs(5)).send_json(body);
            });
        }
        Some(Ok(()))
    }

    fn watches(&self) -> &'static [&'static str] {
        &["key.event", "deck.state"]
    }

    /// Every key event and state change goes to the subscriptions that asked for it.
    fn on_signal(&mut self, name: &str, value: &Value, _host: &mut Host) {
        let (kind, wants): (&str, fn(&Sub) -> bool) = match name {
            "key.event" => ("key", |s| s.keys),
            "deck.state" => ("state", |s| s.state),
            _ => return,
        };
        let now = std::time::Instant::now();
        self.subs.retain(|_, s| s.until > now);
        for url in self.subs.iter().filter(|(_, s)| wants(s)).map(|(u, _)| u.clone()) {
            let mut body = value.clone();
            body["type"] = json!(kind);
            std::thread::spawn(move || {
                let _ = ureq::post(&url).timeout(Duration::from_secs(5)).send_json(body);
            });
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

    fn status(&self) -> Value {
        json!({ "web": {
            "listening": self.server.as_ref().map(|s| s.port),
            "error": self.error,
            "addresses": self.server.as_ref().map(|s| local_addresses(s.port)),
        }})
    }
}

/// The OpenAPI 3.1 description of this API (served without a token at /api/openapi.json).
pub fn openapi() -> Value {
    let err = json!({ "$ref": "#/components/responses/Error" });
    let ok = |desc: &str| json!({ "description": desc, "content": { "application/json": { "schema": { "type": "object" } } } });
    let name = |n: &str, d: &str| json!({ "name": n, "in": "path", "required": true, "description": d, "schema": { "type": "string", "pattern": "^[A-Za-z0-9_.-]{1,128}$" } });
    let key = json!({ "$ref": "#/components/schemas/Key" });
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "D200 Deck Web API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Drive a Ulanzi D200 through D200 Deck: keys by id, signals, notifications, presses and actions. Every endpoint but this description needs the token from the plugin's settings (Packages, Web API).",
        },
        "servers": [{ "url": "/" }],
        "security": [{ "token": [] }],
        "paths": {
            "/api": { "get": { "summary": "What this server offers", "responses": { "200": ok("Name, version, endpoints"), "401": err } } },
            "/api/openapi.json": { "get": { "summary": "This description", "security": [], "responses": { "200": ok("OpenAPI 3.1") } } },
            "/api/state": { "get": { "summary": "What the deck shows and knows: profile, page, display, screensaver, USB, the program in front, Teams, media, Pomodoro, versions", "responses": { "200": ok("The deck state"), "401": err, "503": err } } },
            "/api/signals": { "get": { "summary": "Every signal with its value", "responses": { "200": ok("Object: signal name to value"), "401": err } } },
            "/api/signals/{name}": {
                "parameters": [name("name", "A full signal name, e.g. com.deckling.core:system.cpu (GET) or a name in com.deckling.web (PUT)")],
                "get": { "summary": "One signal", "responses": { "200": ok("{name, value}"), "404": err, "401": err } },
                "put": {
                    "summary": "Set the signal com.deckling.web:<name>",
                    "description": "Themes and icons can bind to it. The body is any JSON value, or {\"value\": ...}.",
                    "requestBody": { "required": true, "content": { "application/json": { "schema": {} } } },
                    "responses": { "200": ok("{signal}"), "400": err, "401": err },
                },
            },
            "/api/keys": { "get": { "summary": "Key ids set over this API, with their labels", "responses": { "200": ok("Object: id to label"), "401": err } } },
            "/api/keys/{id}": {
                "parameters": [name("id", "The key id; put the app Web key on a key and pick it")],
                "put": { "summary": "Describe a key", "requestBody": { "required": true, "content": { "application/json": { "schema": key } } }, "responses": { "200": ok("{key}"), "400": err, "401": err } },
                "delete": { "summary": "Remove a key", "responses": { "200": ok("{removed}"), "404": err, "401": err } },
            },
            "/api/keys/{index}/press": {
                "parameters": [{ "name": "index", "in": "path", "required": true, "description": "0 to 12, 13 is the wide key", "schema": { "type": "integer", "minimum": 0, "maximum": 13 } }],
                "post": {
                    "summary": "Press a key of the page on the device",
                    "requestBody": { "content": { "application/json": { "schema": { "type": "object", "properties": { "event": { "type": "string", "enum": ["press", "long", "double", "down", "up"], "default": "press" } } } } } },
                    "responses": { "200": ok("{pressed}"), "400": err, "401": err },
                },
            },
            "/api/notify": {
                "post": {
                    "summary": "Show a notification on the wide key",
                    "requestBody": { "required": true, "content": { "application/json": { "schema": { "oneOf": [{ "type": "string" }, { "$ref": "#/components/schemas/Notification" }] } } } },
                    "responses": { "200": ok("{shown}"), "400": err, "401": err },
                },
            },
            "/api/actions/{action}": {
                "parameters": [{ "name": "action", "in": "path", "required": true, "description": "An action id, e.g. core:page or com.deckling.core:media", "schema": { "type": "string" } }],
                "post": {
                    "summary": "Run an action",
                    "description": "Actions that start programs or type (com.deckling.core:command, :open, :keys) are refused unless allowed in the plugin's settings.",
                    "requestBody": { "content": { "application/json": { "schema": { "type": "object", "properties": { "params": { "type": "object" } } } } } },
                    "responses": { "200": ok("{ran}"), "403": err, "404": err, "401": err },
                },
            },
            "/api/mqtt": {
                "get": { "summary": "The deck's shared MQTT settings (without the password)", "responses": { "200": ok("{enabled, host, port, username, has_password, tls, base_topic, discovery}"), "401": err } },
                "put": {
                    "summary": "Set up the deck's shared MQTT connection",
                    "description": "Saved to deck.json; the deck connects right away. Home Assistant's integration uses this to hand over its broker.",
                    "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "required": ["host"], "properties": {
                        "host": { "type": "string" }, "port": { "type": "integer" }, "username": { "type": "string" }, "password": { "type": "string" },
                        "tls": { "type": "boolean" }, "base_topic": { "type": "string" }, "enabled": { "type": "boolean", "default": true } } } } } },
                    "responses": { "200": ok("{saved}"), "400": err, "401": err },
                },
            },
            "/api/subscriptions": {
                "get": { "summary": "Webhooks registered over this API", "responses": { "200": ok("[{url, expires_in}]"), "401": err } },
                "post": {
                    "summary": "Register (or renew) a webhook for presses on Web keys",
                    "description": "The deck posts {\"id\", \"event\"} to the URL for every press on a key with the app Web key. A subscription expires after ttl seconds (10 to 3600, default 900); renew it before then.",
                    "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "required": ["url"], "properties": { "url": { "type": "string", "format": "uri" }, "ttl": { "type": "integer", "minimum": 10, "maximum": 3600 },
                        "events": { "type": "array", "items": { "type": "string", "enum": ["presses", "keys", "state"] }, "description": "presses: presses on Web keys ({id, event}); keys: also every key event ({type: key, key, event, profile, page, n}); state: every change of GET /api/state ({type: state, ...})" } } } } } },
                    "responses": { "200": ok("{url, expires_in}"), "400": err, "401": err },
                },
                "delete": {
                    "summary": "Remove a webhook",
                    "requestBody": { "required": true, "content": { "application/json": { "schema": { "type": "object", "required": ["url"], "properties": { "url": { "type": "string" } } } } } },
                    "responses": { "200": ok("{removed}"), "404": err, "401": err },
                },
            },
        },
        "components": {
            "securitySchemes": { "token": { "type": "http", "scheme": "bearer", "description": "The token from Packages, Web API (also accepted as ?token=)" } },
            "responses": { "Error": { "description": "An error", "content": { "application/json": { "schema": { "type": "object", "properties": { "error": { "type": "string" } } } } } } },
            "schemas": {
                "Key": {
                    "type": "object",
                    "properties": {
                        "label": { "type": "string" },
                        "kind": { "type": "string", "enum": ["light", "switch", "sensor", "climate", "lock", "unlock", "cover", "scene", "vacuum", "automation", "presence", "energy"] },
                        "icon": { "type": "string", "description": "An icon name, a full icon id or an emoji; overrides kind" },
                        "value": { "type": "string" },
                        "on": { "type": "boolean" },
                        "color": { "type": "string", "description": "#RRGGBB" },
                        "progress": { "type": "number", "minimum": 0, "maximum": 100 },
                        "background": { "type": "string", "description": "#RRGGBB" },
                    },
                },
                "Notification": {
                    "type": "object",
                    "properties": {
                        "title": { "type": "string" }, "text": { "type": "string" }, "icon": { "type": "string" }, "color": { "type": "string" },
                        "duration": { "type": "number" }, "priority": { "type": "string", "enum": ["normal", "critical"] },
                    },
                },
            },
        },
    })
}

fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 128
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// Where the server can be reached from other machines (best effort, for the settings).
fn local_addresses(port: u16) -> Vec<String> {
    // the address used for the default route: no packets are sent
    let ip = std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("192.0.2.1:9").and_then(|_| s.local_addr()))
        .ok()
        .map(|a| a.ip());
    let mut out = vec![format!("http://127.0.0.1:{port}/api")];
    if let Some(ip) = ip.filter(|ip| !ip.is_loopback() && !ip.is_unspecified()) {
        out.push(format!("http://{ip}:{port}/api"));
    }
    out
}

/// Token check that takes the same time however much of the token matches.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

fn serve(mut s: TcpStream, token: &str, sender: &crate::plugins::Sender) -> std::io::Result<()> {
    let _ = s.set_nonblocking(false);
    s.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut r = BufReader::new(s.try_clone()?);
    let mut first = String::new();
    r.read_line(&mut first)?;
    let mut len = 0usize;
    let mut auth = String::new();
    loop {
        let mut l = String::new();
        if r.read_line(&mut l)? == 0 || l.trim().is_empty() {
            break;
        }
        let (k, v) = l
            .split_once(':')
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
            .unwrap_or_default();
        match k.as_str() {
            "content-length" => len = v.parse().unwrap_or(0),
            "authorization" => auth = v.strip_prefix("Bearer ").unwrap_or(&v).to_string(),
            _ => {}
        }
    }
    let mut it = first.split_whitespace();
    let method = it.next().unwrap_or("").to_ascii_uppercase();
    let target = it.next().unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if auth.is_empty() {
        auth = query
            .split('&')
            .find_map(|kv| kv.strip_prefix("token="))
            .unwrap_or("")
            .to_string();
    }
    // The API description is public: it holds no secrets and lets tools discover the API.
    if method == "GET" && path == "/api/openapi.json" {
        let out = openapi().to_string();
        write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len())?;
        return s.flush();
    }
    let (code, body) = if !same(&auth, token) {
        (401, json!({ "error": "missing or wrong token" }))
    } else if len > MAX_BODY {
        (413, json!({ "error": "body too large" }))
    } else {
        let mut raw = vec![0; len];
        r.read_exact(&mut raw)?;
        let text = String::from_utf8_lossy(&raw).to_string();
        let body = if text.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::String(text))
        };
        let (tx, rx) = mpsc::channel();
        sender.send(Request {
            method,
            path: path.to_string(),
            body,
            reply: tx,
        });
        rx.recv_timeout(Duration::from_secs(5))
            .unwrap_or((503, json!({ "error": "the deck engine did not answer" })))
    };
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        _ => "Service Unavailable",
    };
    let out = body.to_string();
    write!(s, "HTTP/1.1 {code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len())?;
    s.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bind::Signals;

    fn call(
        web: &mut Web,
        s: &mut Signals,
        method: &str,
        path: &str,
        body: Value,
    ) -> ((u16, Value), Vec<HostRequest>) {
        let mut h = Host::new(ID, s, "d200".into());
        let r = web.handle(method, path, body, &mut h);
        (r, h.requests)
    }

    #[test]
    fn keys_signals_and_requests() {
        let (mut web, mut s) = (Web::default(), Signals::default());
        let ((code, _), _) = call(
            &mut web,
            &mut s,
            "PUT",
            "/api/keys/build",
            json!({"label": "Build", "kind": "sensor", "value": "green", "on": true}),
        );
        assert_eq!(code, 200);
        assert_eq!(s.get("com.deckling.web:build.label"), Some(&json!("Build")));
        assert_eq!(s.get("com.deckling.web:build.on"), Some(&json!(true)));
        assert_eq!(
            web.options("ids").unwrap(),
            vec![json!({"label": "Build (build)", "value": "build"})]
        );

        let ((code, _), _) = call(
            &mut web,
            &mut s,
            "PUT",
            "/api/signals/ci.status",
            json!({"value": "failed"}),
        );
        assert_eq!(code, 200);
        assert_eq!(s.get("com.deckling.web:ci.status"), Some(&json!("failed")));
        let ((_, v), _) = call(
            &mut web,
            &mut s,
            "GET",
            "/api/signals/com.deckling.web:ci.status",
            Value::Null,
        );
        assert_eq!(v["value"], "failed");

        let (_, reqs) = call(
            &mut web,
            &mut s,
            "POST",
            "/api/notify",
            json!({"title": "Build", "text": "failed"}),
        );
        assert!(matches!(&reqs[..], [HostRequest::Notify(t)] if t.contains("failed")));
        let (_, reqs) = call(
            &mut web,
            &mut s,
            "POST",
            "/api/keys/3/press",
            json!({"event": "long"}),
        );
        assert!(matches!(
            reqs[..],
            [HostRequest::Press {
                key: 3,
                event: KeyEvent::Long
            }]
        ));
        let ((code, _), _) = call(&mut web, &mut s, "POST", "/api/keys/14/press", Value::Null);
        assert_eq!(code, 400);

        let ((code, _), _) = call(&mut web, &mut s, "DELETE", "/api/keys/build", Value::Null);
        assert_eq!(code, 200);
        assert_eq!(s.get("com.deckling.web:build.label"), None);
        let ((code, _), _) = call(&mut web, &mut s, "GET", "/api/nope", Value::Null);
        assert_eq!(code, 404);
    }

    #[test]
    fn program_actions_need_permission() {
        let (mut web, mut s) = (Web::default(), Signals::default());
        let ((code, _), reqs) = call(
            &mut web,
            &mut s,
            "POST",
            "/api/actions/com.deckling.core:command",
            json!({"params": {"command": "true"}}),
        );
        assert_eq!(code, 403);
        assert!(reqs.is_empty());
        let ((code, _), reqs) = call(
            &mut web,
            &mut s,
            "POST",
            "/api/actions/core:page",
            json!({"params": {"page": "more"}}),
        );
        assert_eq!(code, 200);
        assert!(
            matches!(&reqs[..], [HostRequest::Action(a)] if a.action == "core:page" && a.params["page"] == "more")
        );
        web.settings.allow_programs = true;
        let ((code, _), _) = call(
            &mut web,
            &mut s,
            "POST",
            "/api/actions/com.deckling.core:command",
            json!({"params": {"command": "true"}}),
        );
        assert_eq!(code, 200);
        let ((code, _), _) = call(
            &mut web,
            &mut s,
            "POST",
            "/api/actions/no.such:thing",
            Value::Null,
        );
        assert_eq!(code, 404);
    }

    #[test]
    fn subscriptions() {
        let (mut web, mut s) = (Web::default(), Signals::default());
        let ((code, _), _) = call(&mut web, &mut s, "POST", "/api/subscriptions", json!({"url": "ftp://x"}));
        assert_eq!(code, 400);
        let ((code, v), _) = call(&mut web, &mut s, "POST", "/api/subscriptions", json!({"url": "http://ha.local:8123/api/webhook/abc", "ttl": 99999}));
        assert_eq!((code, v["expires_in"].as_u64()), (200, Some(MAX_TTL)));
        let ((_, list), _) = call(&mut web, &mut s, "GET", "/api/subscriptions", Value::Null);
        assert_eq!(list[0]["url"], "http://ha.local:8123/api/webhook/abc");
        let ((code, _), _) = call(&mut web, &mut s, "DELETE", "/api/subscriptions", json!({"url": "http://ha.local:8123/api/webhook/abc"}));
        assert_eq!(code, 200);
        assert!(web.subs.is_empty());
    }

    #[test]
    fn state_and_state_subscriptions() {
        let mut web = Web::default();
        let mut s = Signals::default();
        let ((code, _), _) = call(&mut web, &mut s, "GET", "/api/state", Value::Null);
        assert_eq!(code, 503);
        s.publish("deck.state", json!({"page": "main", "usb": true}));
        let ((code, v), _) = call(&mut web, &mut s, "GET", "/api/state", Value::Null);
        assert_eq!((code, v["page"].as_str()), (200, Some("main")));
        let ((_, v), _) = call(&mut web, &mut s, "POST", "/api/subscriptions", json!({"url": "http://ha/x", "events": ["presses", "state"]}));
        assert_eq!((v["keys"].as_bool(), v["state"].as_bool()), (Some(false), Some(true)));
        assert!(web.watches().contains(&"deck.state"));
    }

    #[test]
    fn openapi_describes_every_route() {
        let doc = openapi();
        assert_eq!(doc["openapi"], "3.1.0");
        let paths = doc["paths"].as_object().unwrap();
        for p in ["/api/state", "/api/mqtt", "/api", "/api/openapi.json", "/api/signals", "/api/signals/{name}", "/api/keys", "/api/keys/{id}", "/api/keys/{index}/press", "/api/notify", "/api/actions/{action}", "/api/subscriptions"] {
            assert!(paths.contains_key(p), "{p}");
        }
        assert_eq!(doc["paths"]["/api/openapi.json"]["get"]["security"], json!([]));
    }

    #[test]
    fn switching_off_stops_the_server() {
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let (cfg, _) = Config::from_value(json!({
            "version": 6,
            "plugins": {"com.deckling.web": {"config": {"serve": true, "port": port, "token": "t"}}},
            "profiles": [{"name": "Default", "apps": [], "pages": {"main": {}}}],
        }))
        .unwrap();
        std::env::set_var("D200_NO_MDNS", "1");
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut s = Signals::default();
        let mut web = Web::default();
        {
            let mut h = Host::new(ID, &mut s, "d200".into()).with_engine(std::env::temp_dir(), crate::plugins::Sender::new(ID, tx));
            web.configure(&cfg, &mut h);
        }
        assert!(web.server.is_some());
        assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok(), "listening");
        web.stop();
        std::thread::sleep(Duration::from_millis(400));
        assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err(), "stopped");
    }

    #[test]
    fn token_compare() {
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("", "abc"));
    }
}
