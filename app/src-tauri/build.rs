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
    std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("icons_gen.rs"), out).unwrap();

    tauri_build::build()
}
