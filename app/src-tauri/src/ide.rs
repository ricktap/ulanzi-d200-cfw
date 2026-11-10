//! JetBrains IDE debugger detection.
//!
//! JetBrains IDEs expose no API for debugger state, so this looks for debuggee
//! processes started by an IDE: Node with --inspect / --inspect-brk (WebStorm),
//! the JDWP agent (IntelliJ), debugpy / pydevd (PyCharm), dlv (GoLand).

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System, UpdateKind};

const IDE_NAMES: [&str; 11] =
    ["webstorm", "idea", "pycharm", "goland", "phpstorm", "rider", "clion", "rubymine", "datagrip", "rustrover", "fleet"];
const DEBUG_MARKERS: [&str; 6] = ["--inspect", "jdwp", "debugpy", "pydevd", "dlv", "--debug-brk"];

pub struct DebugWatch {
    sys: System,
}

/// A running Steam game.
#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub appid: Option<u32>,
    pub name: String,
    /// Unix seconds the game process started.
    pub started: u64,
}

/// `steamapps/common/<installdir>` -> (appid, name) from the appmanifest files.
fn manifest(steamapps: &std::path::Path, installdir: &str) -> Option<(u32, String)> {
    for e in std::fs::read_dir(steamapps).ok()?.flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if !(n.starts_with("appmanifest_") && n.ends_with(".acf")) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
        let field = |k: &str| {
            text.lines().find_map(|l| {
                let parts: Vec<&str> = l.split('"').collect();
                (parts.len() >= 4 && parts[1].eq_ignore_ascii_case(k)).then(|| parts[3].to_string())
            })
        };
        if field("installdir").is_some_and(|d| d.eq_ignore_ascii_case(installdir)) {
            return Some((field("appid")?.parse().ok()?, field("name").unwrap_or_else(|| installdir.to_string())));
        }
    }
    None
}

/// What a running debug session looks like (for tests).
pub fn is_debug_cmd(cmd: &str) -> bool {
    let c = cmd.to_ascii_lowercase();
    DEBUG_MARKERS.iter().any(|m| c.contains(m))
}

pub fn is_ide_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    IDE_NAMES.iter().any(|i| n == *i || n.starts_with(&format!("{i}.")) || n.contains(&format!("{i}64")))
        || n.contains("jetbrains")
}

impl DebugWatch {
    pub fn new() -> Self {
        DebugWatch { sys: System::new_with_specifics(RefreshKind::nothing()) }
    }

    fn refresh(&mut self) {
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet).with_exe(UpdateKind::OnlyIfNotSet),
        );
    }

    /// The Steam game running now: a process under `steamapps/common/<game>/`.
    pub fn steam_game(&mut self) -> Option<Game> {
        self.refresh();
        let mut best: Option<Game> = None;
        for p in self.sys.processes().values() {
            let Some(exe) = p.exe() else { continue };
            let path = exe.to_string_lossy().replace('\\', "/");
            let Some(i) = path.to_ascii_lowercase().find("/steamapps/common/") else { continue };
            let after = &path[i + "/steamapps/common/".len()..];
            let installdir = after.split('/').next().unwrap_or("").to_string();
            if installdir.is_empty() || installdir.eq_ignore_ascii_case("Steamworks Shared") {
                continue;
            }
            let steamapps = std::path::PathBuf::from(&path[..i + "/steamapps".len()]);
            let (appid, name) = match manifest(&steamapps, &installdir) {
                Some((id, n)) => (Some(id), n),
                None => (None, installdir.clone()),
            };
            let g = Game { appid, name, started: p.start_time() };
            if best.as_ref().is_none_or(|b| g.started < b.started) {
                best = Some(g);
            }
        }
        best
    }

    /// Name of the IDE with an active debug session, if any.
    pub fn active(&mut self) -> Option<String> {
        self.refresh();
        let procs = self.sys.processes();
        let ide_of = |mut pid: Option<Pid>| -> Option<String> {
            for _ in 0..12 {
                let p = procs.get(&pid?)?;
                let name = p.name().to_string_lossy().to_string();
                if is_ide_name(&name) {
                    return Some(name);
                }
                pid = p.parent();
            }
            None
        };
        for p in procs.values() {
            let cmd = p.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" ");
            if is_debug_cmd(&cmd) {
                if let Some(ide) = ide_of(p.parent()) {
                    return Some(ide);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_appmanifest() {
        let dir = std::env::temp_dir().join(format!("d200-steam-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("appmanifest_275850.acf"),
            "\"AppState\"\n{\n\t\"appid\"\t\t\"275850\"\n\t\"name\"\t\t\"No Man's Sky\"\n\t\"installdir\"\t\t\"No Man's Sky\"\n}\n",
        )
        .unwrap();
        assert_eq!(manifest(&dir, "No Man's Sky"), Some((275850, "No Man's Sky".to_string())));
        assert_eq!(manifest(&dir, "Other"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn markers() {
        assert!(is_debug_cmd("/usr/local/bin/node --inspect-brk=127.0.0.1:52011 app.js"));
        assert!(is_debug_cmd("java -agentlib:jdwp=transport=dt_socket,address=127.0.0.1:5005 Main"));
        assert!(!is_debug_cmd("node server.js"));
        assert!(is_ide_name("webstorm"));
        assert!(is_ide_name("idea"));
        assert!(is_ide_name("pycharm64.exe"));
        assert!(!is_ide_name("ideaz"));
    }
}

#[cfg(test)]
mod live {
    /// `cargo test -- --ignored ide_dump`
    #[test]
    #[ignore]
    fn ide_dump() {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System, UpdateKind};
        let mut sys = System::new_with_specifics(RefreshKind::nothing());
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet));
        for (pid, p) in sys.processes() {
            let cmd = p.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" ");
            let name = p.name().to_string_lossy();
            if cmd.contains("inspect") || name.contains("webstorm") {
                println!("{pid} name={name} parent={:?} cmd={cmd}", p.parent());
            }
        }
        println!("active = {:?}", super::DebugWatch::new().active());
    }
}
