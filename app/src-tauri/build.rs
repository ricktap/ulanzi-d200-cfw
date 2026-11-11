use std::fmt::Write;

fn main() {
    // The device firmware is embedded so the app can start it on a D200 by itself.
    println!("cargo:rerun-if-changed=../../bin/d200fw");

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
    for (root, prefix) in [("core", "core"), ("plugins", "plugins"), ("../ui/icons/night-signal", NIGHT_SIGNAL)] {
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
