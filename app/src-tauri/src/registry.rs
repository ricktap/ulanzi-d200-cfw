//! The registry: everything packages contribute (docs/deckling/architecture.md, sections 3 to 7).
//!
//! A package is a folder with a `deckling.json`. This version loads the built-in
//! packages (the `core` plugin with the Night Signal icon set embedded) from files
//! compiled in by `build.rs`; loading packages from disk uses the same loader.
//!
//! Load-time checks follow the icon workbench report: namespaces, missing fallbacks,
//! parts missing from the SVG, properties an icon reads but its roles do not declare,
//! and icon sets drawn against another version of an interface.

use crate::bind;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

use crate::icons::{ICONS, PACKAGE_FILES};

/// Property name -> type (`flag`, `percent`, `number`, `color`, `text`).
pub type Interface = IndexMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// Settings schema, as for apps.
    #[serde(default)]
    pub config: Vec<Value>,
    #[serde(default)]
    pub runtime: Value,
    #[serde(default)]
    pub contributes: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalDef {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDef {
    pub id: String,
    pub name: String,
    /// Parameter schema: fields like app settings (`text`, `toggle`, `select`, `number`).
    #[serde(default)]
    pub params: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDef {
    pub id: String,
    pub name: String,
    /// Default icon, from the plugin's embedded icon set: an id, or a binding that
    /// resolves to one (a plugin that picks the icon per key, like Home Assistant).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<Value>,
    /// Text, value and background for keys drawn with an icon (not a built-in widget);
    /// bindings like `implements`. A label or colour set on the key wins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Core only: a built-in widget drawn while the default icon is chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<String>,
    #[serde(default)]
    pub config: Vec<Value>,
    /// Interface -> property -> binding.
    #[serde(default)]
    pub implements: Map<String, Value>,
    /// Preset actions for a new key.
    #[serde(default)]
    pub on: Value,
}

impl AppDef {
    /// The default icon id without resolving anything: the id, or a binding's fallback.
    pub fn icon_static(&self) -> Option<&str> {
        match self.icon.as_ref()? {
            Value::String(s) => Some(s),
            Value::Object(o) => o.get("fallback").and_then(Value::as_str),
            _ => None,
        }
    }
    /// Roles in the app's order, `button` last (every app fulfils it).
    pub fn roles(&self) -> Vec<String> {
        let mut r: Vec<String> = self.implements.keys().cloned().collect();
        if !r.iter().any(|x| x == "button") {
            r.push("button".into());
        }
        r
    }
    /// Default values of the settings, for a new key.
    pub fn defaults(&self) -> Map<String, Value> {
        self.config
            .iter()
            .filter_map(|f| Some((f.get("key")?.as_str()?.to_string(), f.get("default")?.clone())))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconDef {
    pub id: String,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Part -> attribute -> binding.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub parts: Map<String, Value>,
    #[serde(skip)]
    pub svg: String,
}

#[derive(Debug, Default)]
pub struct Registry {
    pub plugins: Vec<PluginInfo>,
    pub interfaces: IndexMap<String, Interface>,
    pub signals: IndexMap<String, SignalDef>,
    pub actions: IndexMap<String, ActionDef>,
    pub apps: IndexMap<String, AppDef>,
    pub icons: IndexMap<String, IconDef>,
    pub warnings: Vec<String>,
}

/// Does package `pkg` own `name`? Names without a prefix belong to the core.
pub fn owns(pkg: &str, name: &str) -> bool {
    match name.split_once(':') {
        Some((ns, _)) => ns == pkg,
        None => pkg == "core",
    }
}

/// Files of one or more packages: path -> text.
pub trait Files {
    fn read(&self, path: &str) -> Option<String>;
    /// Paths directly or indirectly below `dir/`.
    fn list(&self, dir: &str) -> Vec<String>;
    /// SVG source for `file` in the icon set at `dir`.
    fn svg(&self, dir: &str, file: &str) -> Option<String>;
}

/// The packages compiled into the app.
pub struct Embedded;

impl Files for Embedded {
    fn read(&self, path: &str) -> Option<String> {
        PACKAGE_FILES.iter().find(|(p, _)| *p == path).map(|(_, t)| t.to_string())
    }
    fn list(&self, dir: &str) -> Vec<String> {
        let pre = format!("{}/", dir.trim_end_matches('/'));
        PACKAGE_FILES.iter().filter(|(p, _)| p.starts_with(&pre)).map(|(p, _)| p.to_string()).collect()
    }
    fn svg(&self, dir: &str, file: &str) -> Option<String> {
        if dir != NIGHT_SIGNAL {
            return self.read(&format!("{dir}/svg/{file}"));
        }
        let name = file.strip_suffix(".svg").unwrap_or(file);
        ICONS.binary_search_by(|(n, _)| n.cmp(&name)).ok().map(|i| ICONS[i].1.to_string())
    }
}

/// Night Signal in the embedded files: the icon set of com.deckling.core.
pub const NIGHT_SIGNAL: &str = "plugins/com.deckling.core/iconset";

/// The plugin that holds the built-in apps and Night Signal. Plain icon names
/// ("volume-on") and the old `core:` icon ids resolve to it.
pub const CORE_PLUGIN: &str = "com.deckling.core";

/// The registry of the built-in packages: the fixed core, then every plugin in `plugins/`.
pub fn builtin() -> &'static Registry {
    static R: OnceLock<Registry> = OnceLock::new();
    R.get_or_init(|| {
        let mut r = Registry::default();
        r.load_plugin(&Embedded, "core");
        let mut dirs: Vec<String> = Embedded
            .list("plugins")
            .into_iter()
            .filter_map(|p| p.strip_suffix("/deckling.json").map(str::to_string))
            .filter(|d| d.matches('/').count() == 1)
            .collect();
        dirs.sort();
        for d in dirs {
            r.load_plugin(&Embedded, &d);
        }
        for w in &r.warnings {
            eprintln!("packages: {w}");
        }
        r
    })
}

fn parse<T: serde::de::DeserializeOwned>(files: &dyn Files, path: &str, warnings: &mut Vec<String>) -> Option<T> {
    let text = files.read(path)?;
    match serde_json::from_str(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            warnings.push(format!("{path}: {e}"));
            None
        }
    }
}

impl Registry {
    /// Load the plugin in folder `dir`.
    pub fn load_plugin(&mut self, files: &dyn Files, dir: &str) {
        let Some(info) = parse::<PluginInfo>(files, &format!("{dir}/deckling.json"), &mut self.warnings) else {
            self.warnings.push(format!("{dir}: no deckling.json"));
            return;
        };
        if self.plugins.iter().any(|p| p.id == info.id) {
            self.warnings.push(format!("{dir}: a package with id {} is already installed", info.id));
            return;
        }
        let id = info.id.clone();
        let c = info.contributes.clone();
        let path = |k: &str| c.get(k).and_then(Value::as_str).map(|p| format!("{dir}/{}", p.trim_end_matches('/')));
        if let Some(p) = path("interfaces") {
            let list: Map<String, Value> = parse(files, &p, &mut self.warnings).unwrap_or_default();
            for (name, props) in list {
                if !owns(&id, &name) {
                    self.warnings.push(format!("{id}: interface {name} is outside its namespace"));
                    continue;
                }
                let props: Interface = serde_json::from_value(props).unwrap_or_default();
                self.interfaces.insert(name, props);
            }
        }
        if let Some(p) = path("signals") {
            for s in parse::<Vec<SignalDef>>(files, &p, &mut self.warnings).unwrap_or_default() {
                if owns(&id, &s.name) {
                    self.signals.insert(s.name.clone(), s);
                } else {
                    self.warnings.push(format!("{id}: signal {} is outside its namespace", s.name));
                }
            }
        }
        if let Some(p) = path("actions") {
            for a in parse::<Vec<ActionDef>>(files, &p, &mut self.warnings).unwrap_or_default() {
                if owns(&id, &a.id) {
                    self.actions.insert(a.id.clone(), a);
                } else {
                    self.warnings.push(format!("{id}: action {} is outside its namespace", a.id));
                }
            }
        }
        if let Some(p) = path("apps") {
            for f in files.list(&p).into_iter().filter(|f| f.ends_with(".json")) {
                let Some(a) = parse::<AppDef>(files, &f, &mut self.warnings) else { continue };
                if !owns(&id, &a.id) {
                    self.warnings.push(format!("{id}: app {} is outside its namespace", a.id));
                    continue;
                }
                let mut miss = vec![];
                bind::missing_fallbacks(&Value::Object(a.implements.clone()), &a.id, &mut miss);
                for (k, v) in [("icon", &a.icon), ("label", &a.label), ("value", &a.value), ("background", &a.background)] {
                    if let Some(v) = v {
                        bind::missing_fallbacks(v, &format!("{}.{k}", a.id), &mut miss);
                    }
                }
                for m in miss {
                    self.warnings.push(format!("{m}: binding without fallback"));
                }
                self.apps.insert(a.id.clone(), a);
            }
        }
        self.plugins.push(info);
        if let Some(p) = path("iconset") {
            self.load_iconset(files, &p, &id);
        }
    }

    /// Load an icon set (standalone, or embedded in plugin `owner`).
    pub fn load_iconset(&mut self, files: &dyn Files, dir: &str, owner: &str) {
        let info = parse::<PluginInfo>(files, &format!("{dir}/deckling.json"), &mut self.warnings);
        let ns = info.as_ref().map(|i| i.id.clone()).unwrap_or_else(|| owner.to_string());
        let c = info.as_ref().map(|i| i.contributes.clone()).unwrap_or_default();
        let sub = |k: &str, d: &str| format!("{dir}/{}", c.get(k).and_then(Value::as_str).unwrap_or(d).trim_end_matches('/'));

        // Interfaces the set was drawn against: compare with what is installed.
        for f in files.list(&sub("interfaces", "interfaces/")) {
            let copy: Map<String, Value> = parse(files, &f, &mut self.warnings).unwrap_or_default();
            for (name, props) in copy {
                let props: Interface = serde_json::from_value(props).unwrap_or_default();
                match self.interfaces.get(&name) {
                    Some(have) if *have != props => {
                        self.warnings.push(format!("icon set {ns} was drawn for another version of {name}"))
                    }
                    Some(_) => {}
                    // Not installed: remember the copy so icons can be checked and offered.
                    None => {
                        self.interfaces.insert(name, props);
                    }
                }
            }
        }

        let mut bound: IndexMap<String, IconDef> = IndexMap::new();
        for f in files.list(&sub("icons", "icons/")).into_iter().filter(|f| f.ends_with(".json")) {
            if let Some(d) = parse::<IconDef>(files, &f, &mut self.warnings) {
                bound.insert(d.file.clone().unwrap_or_default(), d);
            }
        }
        let svg_dir = sub("svg", "svg/");
        // Every SVG is an icon; without a binding file it only fulfils `button`.
        let names: Vec<String> = if dir == NIGHT_SIGNAL {
            ICONS.iter().map(|(n, _)| format!("{n}.svg")).collect()
        } else {
            files.list(&svg_dir).into_iter().filter_map(|p| p.rsplit('/').next().map(str::to_string)).filter(|n| n.ends_with(".svg")).collect()
        };
        for file in names {
            let Some(svg) = files.svg(dir, &file) else { continue };
            let mut def = bound.shift_remove(&file).unwrap_or_else(|| IconDef {
                id: format!("{ns}:{}", file.trim_end_matches(".svg")),
                roles: vec!["button".into()],
                file: Some(file.clone()),
                parts: Map::new(),
                svg: String::new(),
            });
            def.svg = svg;
            if !owns(&ns, &def.id) {
                self.warnings.push(format!("{ns}: icon {} is outside its namespace", def.id));
                continue;
            }
            for w in check_icon(&def, &self.interfaces) {
                self.warnings.push(format!("{}: {w}", def.id));
            }
            self.icons.insert(def.id.clone(), def);
        }
        for (file, d) in bound {
            self.warnings.push(format!("{}: SVG {file} not found", d.id));
        }
    }

    pub fn app(&self, id: &str) -> Option<&AppDef> {
        self.apps.get(id)
    }

    /// An icon by id. Night Signal names without a namespace, or with the old `core:`,
    /// are accepted too (remote keys and cards from Home Assistant send plain names).
    pub fn icon(&self, id: &str) -> Option<&IconDef> {
        let id = id.trim();
        self.icons.get(id).or_else(|| {
            let name = id.strip_prefix("core:").unwrap_or(id);
            if name.contains(':') {
                None
            } else {
                self.icons.get(&format!("{CORE_PLUGIN}:{name}"))
            }
        })
    }

    /// Icons offered for an app: those sharing a role, richest shared role first.
    pub fn icons_for(&self, app: &AppDef) -> Vec<(&IconDef, String)> {
        let roles = app.roles();
        let mut out: Vec<(usize, &IconDef, String)> = self
            .icons
            .values()
            .filter_map(|i| {
                let (rank, role) = roles.iter().enumerate().find(|(_, r)| i.roles.contains(r))?;
                Some((rank, i, role.clone()))
            })
            .collect();
        out.sort_by_key(|(rank, _, _)| *rank);
        out.into_iter().map(|(_, i, r)| (i, r)).collect()
    }

    /// Everything the configurator needs, as JSON (Tauri command `catalog`).
    pub fn catalog(&self) -> Value {
        json!({
            "plugins": self.plugins,
            "interfaces": self.interfaces,
            "signals": self.signals.values().collect::<Vec<_>>(),
            "actions": self.actions.values().collect::<Vec<_>>(),
            "apps": self.apps.values().map(|a| {
                let mut v = serde_json::to_value(a).unwrap_or_default();
                v["roles"] = json!(a.roles());
                // the UI shows the static default; a bound icon is resolved per key
                v["icon"] = json!(a.icon_static());
                v
            }).collect::<Vec<_>>(),
            "icons": self.icons.values().map(|i| json!({"id": i.id, "roles": i.roles, "bound": !i.parts.is_empty()})).collect::<Vec<_>>(),
            "warnings": self.warnings,
        })
    }
}

/// The workbench checks for one icon.
pub fn check_icon(def: &IconDef, interfaces: &IndexMap<String, Interface>) -> Vec<String> {
    let mut out = vec![];
    for (part, attrs) in &def.parts {
        let has_part = def.svg.match_indices("data-part=\"").any(|(i, _)| {
            let rest = &def.svg[i + 11..];
            rest[..rest.find('"').unwrap_or(0)].split_whitespace().any(|p| p == part)
        });
        if !has_part {
            out.push(format!("part {part} is not in the SVG"));
        }
        let mut miss = vec![];
        bind::missing_fallbacks(attrs, &format!("parts.{part}"), &mut miss);
        out.extend(miss.into_iter().map(|m| format!("{m}: binding without fallback")));
        let mut props = vec![];
        collect_props(attrs, &mut props);
        for p in props {
            let Some((role, key)) = p.rsplit_once('.') else { continue };
            if !def.roles.iter().any(|r| r == role) {
                out.push(format!("reads {p}, but {role} is not one of its roles"));
            } else if interfaces.get(role).is_some_and(|i| !i.contains_key(key)) {
                out.push(format!("reads {p}, which interface {role} does not have"));
            }
        }
    }
    out
}

fn collect_props(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            if let Some(p) = o.get("prop").and_then(Value::as_str) {
                out.push(p.to_string());
            }
            o.values().for_each(|x| collect_props(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| collect_props(x, out)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn builtin_packages_load_cleanly() {
        let r = builtin();
        assert!(r.warnings.is_empty(), "{:#?}", r.warnings);
        assert!(r.icon("com.deckling.ha:light").is_some(), "the HA plugin's own icon set");
        assert!(r.app("com.deckling.ha:entity").unwrap().icon.as_ref().unwrap().is_object(), "HA picks the icon per key");
        let ids: Vec<&str> = r.plugins.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["core", "com.deckling.core", "com.deckling.ha", "com.deckling.jetbrains", "com.deckling.teams"]);
        assert!(r.apps.contains_key("com.deckling.core:clock"));
        assert!(r.apps.contains_key("com.deckling.teams:mute"));
        assert!(r.apps.contains_key("com.deckling.jetbrains:debugger"));
        assert!(r.actions.contains_key("core:page"));
        assert!(r.actions.contains_key("com.deckling.ha:mqtt"));
        assert!(r.signals.contains_key("time.second"));
        assert!(r.signals.contains_key("com.deckling.teams:mic_on"));
        assert_eq!(r.icons.keys().filter(|i| i.starts_with("com.deckling.core:")).count(), 218);
        assert_eq!(r.icons.keys().filter(|i| i.starts_with("com.deckling.ha:")).count(), crate::ha::ICONS.len());
        assert_eq!(r.icon("com.deckling.core:light").unwrap().roles, vec!["rgb", "toggle", "button"]);
        assert!(r.icon("volume-on").is_some(), "plain names still resolve");
        assert!(r.icon("core:volume-on").is_some(), "old core: ids still resolve");
        for a in r.apps.values() {
            if let Some(i) = a.icon_static() {
                assert!(r.icon(i).is_some(), "{}: default icon {i} missing", a.id);
            }
        }
    }

    #[test]
    fn icons_offered_by_shared_role() {
        let r = builtin();
        let offered = r.icons_for(r.app("com.deckling.core:clock").unwrap());
        assert_eq!(offered[0].0.id, "com.deckling.core:analog-clock");
        assert_eq!(offered[0].1, "clock");
        // plain icons come last, sharing only `button`
        assert_eq!(offered.last().unwrap().1, "button");
    }

    /// Packages from anywhere (here: a map) go through the same loader and checks.
    struct Map2(HashMap<&'static str, &'static str>);
    impl Files for Map2 {
        fn read(&self, p: &str) -> Option<String> {
            self.0.get(p).map(|s| s.to_string())
        }
        fn list(&self, dir: &str) -> Vec<String> {
            let pre = format!("{}/", dir.trim_end_matches('/'));
            self.0.keys().filter(|k| k.starts_with(&pre)).map(|k| k.to_string()).collect()
        }
        fn svg(&self, dir: &str, file: &str) -> Option<String> {
            self.read(&format!("{dir}/svg/{file}"))
        }
    }

    #[test]
    fn checks_namespaces_and_icons() {
        let files = Map2(HashMap::from([
            ("hue/deckling.json", r#"{"id": "com.ricktap.hue", "type": "plugin", "name": "Hue",
                "contributes": {"interfaces": "interfaces.json", "apps": "apps/", "signals": "signals.json"}}"#),
            ("hue/interfaces.json", r#"{"com.ricktap.hue:rgb_light": {"on": "flag"}, "toggle": {"x": "flag"}}"#),
            ("hue/signals.json", r#"[{"name": "com.ricktap.hue:lamp.power", "type": "flag"}, {"name": "time.second", "type": "number"}]"#),
            ("hue/apps/light.json", r#"{"id": "com.ricktap.hue:light", "name": "Light",
                "implements": {"toggle": {"on": {"bind": "com.ricktap.hue:lamp.power"}}}}"#),
            ("neon/deckling.json", r#"{"id": "com.jemand.neon", "type": "iconset", "name": "Neon"}"#),
            ("neon/icons/bulb.json", r##"{"id": "com.jemand.neon:bulb", "roles": ["toggle", "button"], "file": "bulb.svg",
                "parts": {"glass": {"visible": {"prop": "toggle.on", "fallback": false}, "fill": {"prop": "rgb.color", "fallback": "#FFF"}},
                          "ghost": {"stroke": {"tone": "base"}}}}"##),
            ("neon/interfaces/hue.json", r#"{"com.ricktap.hue:rgb_light": {"on": "flag", "color": "color"}}"#),
            ("neon/svg/bulb.svg", r#"<svg viewBox="0 0 64 64"><circle data-part="glass outline" r="9"/></svg>"#),
        ]));
        let mut r = Registry::default();
        r.load_plugin(&files, "hue");
        r.load_iconset(&files, "neon", "");
        let w = r.warnings.join("\n");
        assert!(w.contains("interface toggle is outside its namespace"), "{w}");
        assert!(w.contains("signal time.second is outside its namespace"), "{w}");
        assert!(w.contains("com.ricktap.hue:light.toggle.on: binding without fallback"), "{w}");
        assert!(w.contains("reads rgb.color, but rgb is not one of its roles"), "{w}");
        assert!(w.contains("part ghost is not in the SVG"), "{w}");
        assert!(w.contains("icon set com.jemand.neon was drawn for another version of com.ricktap.hue:rgb_light"), "{w}");
        assert!(r.icon("com.jemand.neon:bulb").is_some());
    }
}
