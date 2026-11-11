//! The real UI in a normal browser, against the real engine (for development and UI tests):
//!   D200_DIRECT=1 D200_PORT=7399 cargo run --example uibridge -- /path/deck.json 8790
//! Serves app/ui with a small shim for `window.__TAURI__`: commands go to
//! POST /invoke/<command>, events come from GET /events?since=<n>.
use d200_deck_lib::config::{Config, KeyEvent};
use d200_deck_lib::engine::{Engine, Ui};
use d200_deck_lib::render::Notification;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const SHIM: &str = r#"
(() => {
  const listeners = {};
  let since = 0;
  const poll = async () => {
    try {
      const r = await fetch('/events?since=' + since);
      const list = await r.json();
      for (const [n, name, payload] of list) {
        since = n + 1;
        for (const fn of listeners[name] || []) fn({ payload });
      }
    } catch (e) {}
    setTimeout(poll, 300);
  };
  poll();
  window.__TAURI__ = {
    core: { invoke: async (cmd, args) => {
      const r = await fetch('/invoke/' + cmd, { method: 'POST', body: JSON.stringify(args || {}) });
      const v = await r.json();
      if (!r.ok) throw v;
      return v;
    } },
    event: { listen: async (name, fn) => { (listeners[name] ||= []).push(fn); return () => {}; } },
  };
})();
"#;

fn main() {
    let path = PathBuf::from(std::env::args().nth(1).expect("config path"));
    let port = std::env::args().nth(2).unwrap_or_else(|| "8790".into());
    let events: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
    let ev = events.clone();
    let emit = Box::new(move |name: &str, v: Value| ev.lock().unwrap().push((name.to_string(), v)));
    let h = Engine::spawn(path, emit, d200_deck_lib::FIRMWARE);
    let ui = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ui");
    let listener = TcpListener::bind(format!("127.0.0.1:{port}")).expect("bind");
    println!("http://127.0.0.1:{port}/");
    for stream in listener.incoming().flatten() {
        let (h, events, ui) = (h.clone(), events.clone(), ui.clone());
        std::thread::spawn(move || {
            let _ = serve(stream, &h, &events, &ui);
        });
    }
}

fn serve(mut s: TcpStream, h: &d200_deck_lib::engine::Handle, events: &Mutex<Vec<(String, Value)>>, ui: &std::path::Path) -> std::io::Result<()> {
    let mut r = BufReader::new(s.try_clone()?);
    let mut first = String::new();
    r.read_line(&mut first)?;
    let mut len = 0usize;
    loop {
        let mut l = String::new();
        r.read_line(&mut l)?;
        if l.trim().is_empty() {
            break;
        }
        if let Some(v) = l.to_ascii_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body)?;
    let target = first.split_whitespace().nth(1).unwrap_or("/").to_string();
    let (status, ctype, out): (&str, &str, Vec<u8>) = if let Some(cmd) = target.strip_prefix("/invoke/") {
        let args: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
        match invoke(h, cmd, &args) {
            Ok(v) => ("200 OK", "application/json", v.to_string().into_bytes()),
            Err(e) => ("500 Error", "application/json", json!(e).to_string().into_bytes()),
        }
    } else if let Some(q) = target.strip_prefix("/events?since=") {
        let since: usize = q.parse().unwrap_or(0);
        let list: Vec<Value> = events.lock().unwrap().iter().enumerate().skip(since).map(|(i, (n, v))| json!([i, n, v])).collect();
        ("200 OK", "application/json", json!(list).to_string().into_bytes())
    } else {
        let rel = if target == "/" { "index.html" } else { target.trim_start_matches('/') };
        let file = ui.join(rel.split('?').next().unwrap_or(rel));
        match std::fs::read(&file) {
            Ok(mut data) => {
                if rel == "index.html" {
                    let html = String::from_utf8_lossy(&data).replace("<script type=\"module\"", &format!("<script>{SHIM}</script><script type=\"module\""));
                    data = html.into_bytes();
                }
                let ct = match file.extension().and_then(|e| e.to_str()) {
                    Some("html") => "text/html; charset=utf-8",
                    Some("js") => "text/javascript",
                    Some("css") => "text/css",
                    Some("svg") => "image/svg+xml",
                    Some("json") => "application/json",
                    _ => "application/octet-stream",
                };
                ("200 OK", ct, data)
            }
            Err(_) => ("404 Not Found", "text/plain", b"not found".to_vec()),
        }
    };
    write!(s, "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", out.len())?;
    s.write_all(&out)
}

fn arg<T: serde::de::DeserializeOwned>(a: &Value, k: &str) -> Result<T, String> {
    serde_json::from_value(a.get(k).cloned().unwrap_or(Value::Null)).map_err(|e| format!("{k}: {e}"))
}

fn invoke(h: &d200_deck_lib::engine::Handle, cmd: &str, a: &Value) -> Result<Value, String> {
    Ok(match cmd {
        "get_state" => h.ask(Ui::Snapshot)?,
        "save_config" => {
            let (config, _) = Config::from_value(a["config"].clone())?;
            h.ask(|r| Ui::Save(Box::new(config), r))??
        }
        "import_config" => {
            let cfg = Config::parse(a["json"].as_str().unwrap_or("")).map_err(|e| e.to_string())?;
            h.ask(|r| Ui::Save(Box::new(cfg), r))??
        }
        "set_view" => {
            h.send(Ui::View { profile: arg(a, "profile")?, page: arg(a, "page")?, remote: arg(a, "remote")? });
            Value::Null
        }
        "preview" => json!(h.ask(|reply| Ui::Preview { profile: arg(a, "profile").unwrap_or(0), page: arg(a, "page").unwrap_or_default(), remote: arg(a, "remote").unwrap_or(false), key: arg(a, "key").unwrap_or(0), reply })??),
        "trigger" => {
            let event: KeyEvent = arg(a, "event")?;
            h.send(Ui::Trigger { key: arg(a, "key")?, event });
            Value::Null
        }
        "notify" => {
            let n: Notification = arg(a, "notification")?;
            h.send(Ui::Notify(n));
            Value::Null
        }
        "catalog" => d200_deck_lib::registry::builtin().catalog(),
        "options" => json!(h.ask(|reply| Ui::Options { source: arg(a, "source").unwrap_or_default(), profile: arg(a, "profile").unwrap_or(0), reply })?),
        "signals" => h.ask(Ui::Signals)?,
        "preview_icon" => json!(h.ask(|reply| Ui::PreviewIcon { icon: arg(a, "icon").unwrap_or_default(), profile: arg(a, "profile").unwrap_or(0), page: arg(a, "page").unwrap_or_default(), key: arg(a, "key").unwrap_or(0), reply })??),
        "icon_set" => serde_json::from_str::<Value>(d200_deck_lib::icons::MANIFEST).map(|m| m["icons"].clone()).unwrap_or_default(),
        "running_apps" => json!([{ "id": "com.microsoft.teams2", "name": "Microsoft Teams" }, { "id": "com.jetbrains.WebStorm", "name": "WebStorm" }]),
        "autostart_get" => json!(false),
        "app_version" => json!(env!("CARGO_PKG_VERSION")),
        "check_update" => Value::Null,
        _ => Value::Null,
    })
}
