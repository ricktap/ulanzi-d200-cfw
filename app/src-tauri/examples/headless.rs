//! Runs the deck engine without the UI (for testing against a simulator):
//!   D200_DIRECT=1 D200_PORT=7399 cargo run --example headless -- /path/deck.json
//! stdin commands: state | view <profile#> <page> [remote] | preview <key> |
//!                 trigger <key> <down|up|press|long> | notify <text>
use d200_deck_lib::config::KeyEvent;
use d200_deck_lib::engine::{Engine, Ui};
use d200_deck_lib::render::Notification;
use std::path::PathBuf;

fn main() {
    let path = PathBuf::from(std::env::args().nth(1).expect("config path"));
    let emit = Box::new(|name: &str, v: serde_json::Value| println!("event {name} {v}"));
    let h = Engine::spawn(path, emit, d200_deck_lib::FIRMWARE);
    let mut line = String::new();
    loop {
        line.clear();
        if std::io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.as_slice() {
            ["state"] => println!("state {}", h.ask(Ui::Snapshot).unwrap()),
            ["view", p, page, rest @ ..] => h.send(Ui::View { profile: p.parse().unwrap(), page: page.to_string(), remote: !rest.is_empty() }),
            ["preview", k] => {
                let r = h.ask(|reply| Ui::Preview { profile: 0, page: "main".into(), remote: false, key: k.parse().unwrap(), reply });
                println!("preview {:?}", r.map(|x| x.map(|s| s.len())));
            }
            ["trigger", k, ev] => {
                let event: KeyEvent = serde_json::from_value(serde_json::json!(ev)).unwrap();
                h.send(Ui::Trigger { key: k.parse().unwrap(), event });
            }
            ["saver", on] => h.send(Ui::Screensaver(*on == "on")),
            ["notify", rest @ ..] => h.send(Ui::Notify(Notification::simple(&rest.join(" "), Some("🔔"), None))),
            _ => println!("?"),
        }
    }
}
