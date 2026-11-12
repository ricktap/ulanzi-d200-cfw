use std::fmt::Write;

fn main() {
    // The device firmware is embedded so the app can start it on a D200 by itself.
    println!("cargo:rerun-if-changed=../../bin/d200fw");
    // The embedded firmware's version (bin/d200fw is built from device/), to upgrade older
    // d200fw on a connected D200.
    println!("cargo:rerun-if-changed=../../device/Cargo.toml");
    let dev = std::fs::read_to_string("../../device/Cargo.toml").expect("device/Cargo.toml");
    let ver = dev.lines().find_map(|l| l.strip_prefix("version = ")).map(|v| v.trim_matches('"').to_string()).expect("device version");
    println!("cargo:rustc-env=D200FW_VERSION={ver}");

    // Embed the Night Signal icon set (also served to the UI from ui/icons).
    let dir = std::path::Path::new("../ui/icons/night-signal/svg");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("icon set missing")
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".svg")).map(str::to_string))
        .collect();
    names.sort();
    let mut out = String::from("pub static ICONS: &[(&str, &str)] = &[\n");
    for n in &names {
        let path = std::fs::canonicalize(dir.join(format!("{n}.svg"))).unwrap();
        writeln!(out, "    ({n:?}, include_str!({:?})),", path.display().to_string()).unwrap();
    }
    out += "];\n";

    // The core plugin and the Night Signal icon set as package files, parsed at
    // runtime by the package loader (registry.rs): (package path, contents).
    out += "pub static PACKAGE_FILES: &[(&str, &str)] = &[\n";
    // Each built-in plugin keeps its package data next to its code, in
    // src/plugins/<name>/package; it is embedded as plugins/<package id>.
    let mut roots: Vec<(String, String)> = vec![("core".into(), "core".into()), ("themes".into(), "themes".into())];
    println!("cargo:rerun-if-changed=src/plugins");
    let mut pkgs: Vec<_> = std::fs::read_dir("src/plugins").expect("src/plugins").flatten().map(|e| e.path().join("package")).filter(|p| p.is_dir()).collect();
    pkgs.sort();
    for p in pkgs {
        let manifest = std::fs::read_to_string(p.join("deckling.json")).expect("package/deckling.json");
        let id = package_id(&manifest).unwrap_or_else(|| panic!("{}: no id", p.display()));
        roots.push((p.to_str().unwrap().replace('\\', "/"), format!("plugins/{id}")));
    }
    roots.push(("../ui/icons/night-signal".into(), NIGHT_SIGNAL.into()));
    for (root, prefix) in &roots {
        let (root, prefix) = (root.as_str(), prefix.as_str());
        println!("cargo:rerun-if-changed={root}");
        let mut files = vec![];
        // Night Signal's SVGs are in ICONS already; other icon sets bring theirs as package files.
        walk(std::path::Path::new(root), &mut files, root != "../ui/icons/night-signal");
        files.sort();
        for f in files {
            let rel = f.strip_prefix(root).unwrap().to_str().unwrap().replace('\\', "/");
            if !(rel.ends_with(".json") || rel.ends_with(".svg")) || rel.starts_with("manifest") {
                continue;
            }
            let abs = std::fs::canonicalize(&f).unwrap();
            writeln!(out, "    ({:?}, include_str!({:?})),", format!("{prefix}/{rel}"), abs.display().to_string()).unwrap();
        }
    }
    out += "];\n";
    std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("icons_gen.rs"), out).unwrap();

    tauri_build::build()
}

/// `"id": "..."` of a package manifest (no JSON parser in the build script).
fn package_id(manifest: &str) -> Option<String> {
    let at = manifest.find("\"id\"")?;
    let rest = &manifest[at + 4..];
    let start = rest.find('"')? + 1;
    let end = start + rest[start..].find('"')?;
    Some(rest[start..end].to_string())
}

/// Where Night Signal sits in the package files: the icon set embedded in com.deckling.core.
const NIGHT_SIGNAL: &str = "plugins/com.deckling.core/iconset";

fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>, svg: bool) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            if svg || p.file_name().is_some_and(|n| n != "svg") {
                walk(&p, out, svg);
            }
        } else {
            out.push(p);
        }
    }
}
