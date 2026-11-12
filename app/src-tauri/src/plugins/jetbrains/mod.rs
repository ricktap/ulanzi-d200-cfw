//! com.deckling.jetbrains: whether a JetBrains IDE is debugging (process scan in `ide.rs`).
//! Publishes `debugger.active` and `debugger.name`; the debugger app's widget and its
//! press handling read them.

pub mod ide;

use crate::config::{AppKind, Config};
use crate::plugins::jetbrains::ide::DebugWatch;
use crate::plugins::{Host, Plugin};
use serde_json::{json, Value};

pub const ID: &str = "com.deckling.jetbrains";

#[derive(Default)]
pub struct JetBrains {
    watch: Option<DebugWatch>,
    wanted: bool,
}

impl Plugin for JetBrains {
    fn id(&self) -> &'static str {
        ID
    }

    fn configure(&mut self, cfg: &Config, host: &mut Host) {
        self.wanted = cfg.uses(&[AppKind::Debugger], |_| false);
        if !self.wanted {
            self.watch = None;
            host.clear("debugger.name");
            host.publish("debugger.active", json!(false));
        }
    }

    fn tick(&mut self, ticks: u64, _cfg: &Config, host: &mut Host) {
        if !self.wanted || ticks % 2 != 0 {
            return;
        }
        // Scan the processes only while an IDE is in front; keep the last state otherwise.
        if host.front_known && !host.front.as_ref().is_some_and(crate::plugins::jetbrains::ide::is_ide_app) {
            return;
        }
        let active = self.watch.get_or_insert_with(DebugWatch::new).active();
        host.publish("debugger.active", json!(active.is_some()));
        match active {
            Some(name) => host.publish("debugger.name", json!(name)),
            None => host.clear("debugger.name"),
        };
    }

    fn status(&self) -> Value {
        Value::Null
    }
}

/// What the core reads: the name of the IDE that is debugging.
pub fn active(s: &crate::bind::Signals) -> Option<String> {
    s.get(&format!("{ID}:debugger.name")).and_then(Value::as_str).map(str::to_string)
}
