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
const PROGRAM_ACTIONS: [&str; 3] = [
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
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

#[derive(Default)]
pub struct Web {
    settings: Settings,
    server: Option<Server>,
    error: Option<String>,
    /// id -> label, for the id picker.
    ids: BTreeMap<String, String>,
}

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
        self.server = Some(Server {
            stop,
            port: s.port,
            token: s.token.clone(),
        });
    }

    fn handle(&mut self, method: &str, path: &str, body: Value, host: &mut Host) -> (u16, Value) {
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        match (method, parts.as_slice()) {
            ("GET", ["api"]) => (
                200,
                json!({
                    "name": "D200 Deck", "version": env!("CARGO_PKG_VERSION"),
                    "endpoints": ["GET /api/signals", "GET|PUT /api/signals/<name>", "GET /api/keys", "PUT|DELETE /api/keys/<id>",
                                  "POST /api/notify", "POST /api/actions/<action>", "POST /api/keys/<0-13>/press"],
                }),
            ),
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
        let s = Settings::from(cfg);
        if s != self.settings || self.server.is_none() {
            self.settings = s;
            self.start(host);
        }
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
        let url = self.settings.webhook.clone();
        if url.starts_with("http://") || url.starts_with("https://") {
            std::thread::spawn(move || {
                let _ = ureq::post(&url)
                    .timeout(Duration::from_secs(5))
                    .send_json(json!({ "id": id, "event": event }));
            });
        }
        Some(Ok(()))
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
    fn token_compare() {
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd"));
        assert!(!same("", "abc"));
    }
}
