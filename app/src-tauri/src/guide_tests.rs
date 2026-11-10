//! The four running examples of docs/deckling/developer-guide.html, rebuilt from the
//! files on that page (testdata/guide/). `expected.json` holds what the page's own
//! JavaScript `resolve()` returns for each case; the Rust resolver must return the same.

use crate::bind::{self, Ctx, Reads, Signals};
use crate::registry::{check_icon, IconDef, Interface};
use indexmap::IndexMap;
use serde_json::{Map, Value};

fn fixture(name: &str) -> Value {
    let path = format!("{}/src/testdata/guide/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    if name.ends_with(".svg") {
        Value::String(text)
    } else {
        serde_json::from_str(&text).unwrap()
    }
}

fn obj(v: &Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

#[test]
fn examples_resolve_like_the_reference() {
    let exp = fixture("expected.json");
    let partial_order = ["theme-casing", "theme-rainbow", "theme-huesync"];
    let mut checked = 0;
    for case in exp["cases"].as_array().unwrap() {
        let mut signals = Signals::default();
        for (k, v) in obj(&exp["signals"]).into_iter().chain(obj(&case["set"])) {
            signals.publish(&k, v);
        }
        for d in case["drop"].as_array().unwrap() {
            signals.clear(d.as_str().unwrap());
        }
        // one full theme, then the active overlays in list order; later ones win
        let full = fixture(&format!("theme-{}.json", case["theme"].as_str().unwrap()));
        let mut theme = obj(&full["theme"]["variables"]);
        let active: Vec<&str> = case["overlays"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        for p in partial_order {
            let t = fixture(&format!("{p}.json"));
            if active.contains(&t["id"].as_str().unwrap()) {
                theme.extend(obj(&t["theme"]["variables"]));
            }
        }
        let app = fixture(&format!("{}.json", case["app"].as_str().unwrap()));
        let implements = obj(&app["implements"]);
        let config = obj(&case["config"]);
        let ctx = Ctx { implements: Some(&implements), props: None, config: &config, signals: &signals, theme: &theme };
        let icon = fixture(&format!("{}.json", case["icon"].as_str().unwrap()));
        for (part, attrs) in obj(&icon["parts"]) {
            for (attr, spec) in obj(&attrs) {
                let got = bind::resolve(&spec, &ctx, &mut Reads::default()).unwrap_or(Value::Null);
                let want = &case["expect"][&part][&attr];
                assert_eq!(&got, want, "{} / {} / {part}.{attr}", case["name"], case["icon"]);
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "only {checked} values compared");
}

/// The page's icons pass the load-time checks against the page's interfaces.
/// Its apps bind without `fallback`, which architecture rule 7 requires: listed here
/// so a fix to the examples shows up.
#[test]
fn example_packages_pass_the_checks() {
    let mut interfaces: IndexMap<String, Interface> = IndexMap::new();
    for f in ["core-interfaces", "hue-interfaces", "bambu-interfaces", "media-interfaces"] {
        for (name, props) in obj(&fixture(&format!("{f}.json"))) {
            interfaces.insert(name, serde_json::from_value(props).unwrap());
        }
    }
    for name in ["hue-bulb", "neon-bulb", "neon-switch", "neon-lamp", "neon-dial", "neon-spools", "bambu-rings", "core-clock", "media-disc"] {
        let mut def: IconDef = serde_json::from_value(fixture(&format!("icon-{name}.json"))).unwrap();
        def.svg = fixture(&format!("svg-{name}.svg")).as_str().unwrap().to_string();
        assert_eq!(check_icon(&def, &interfaces), Vec::<String>::new(), "{name}");
    }
    let mut missing = vec![];
    for a in ["app-hue", "app-clock", "app-ams", "app-media"] {
        let app = fixture(&format!("{a}.json"));
        bind::missing_fallbacks(&app["implements"], app["id"].as_str().unwrap(), &mut missing);
    }
    // every app binding on the page: hue 5, clock 3, AMS 8, media 6
    assert_eq!(missing.len(), 22, "{missing:#?}");
}
