//! Client for the d200fw line protocol (see the project README), plus adb helpers.

use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 7300;

/// Port of d200fw. `D200_PORT` overrides it (used for testing against a simulator).
pub fn port() -> u16 {
    std::env::var("D200_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(DEFAULT_PORT)
}
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

pub enum DevEvent {
    Key { dev: u64, key: usize, down: bool },
    Closed { dev: u64 },
}

// ------------------------------------------------------------------ adb

/// Find adb. GUI apps on macOS do not get the shell's PATH, so look in the
/// usual install locations as well.
pub fn adb_path() -> Option<PathBuf> {
    static CACHE: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    let mut c = cache.lock().unwrap();
    if let Some(p) = c.as_ref().filter(|p| p.exists()) {
        return Some(p.clone());
    }
    let exe = if cfg!(windows) { "adb.exe" } else { "adb" };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
    if let Some(h) = &home {
        dirs.push(h.join("platform-tools"));
        dirs.push(h.join("Library/Android/sdk/platform-tools"));
        dirs.push(h.join("Android/Sdk/platform-tools"));
        dirs.push(h.join("AppData/Local/Android/Sdk/platform-tools"));
    }
    let found = dirs.into_iter().map(|d| d.join(exe)).find(|p| p.is_file());
    *c = found.clone();
    found
}

pub fn adb(adb: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new(adb);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().map_err(|e| format!("adb: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).replace('\r', "");
    if out.status.success() {
        Ok(text)
    } else {
        Err(format!("adb {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// True when a device is attached and authorised.
/// Run `adb track-devices` and call `changed` whenever the device list changes (also
/// once at start). Restarts it if adb goes away. Runs until `changed` returns false.
pub fn track_devices(adb_bin: PathBuf, changed: impl Fn() -> bool + Send + 'static) {
    use std::io::Read;
    std::thread::spawn(move || loop {
        let child = Command::new(&adb_bin)
            .arg("track-devices")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn();
        if let Ok(mut child) = child {
            if let Some(mut out) = child.stdout.take() {
                // messages: 4 hex digits of length, then that many bytes of "serial\tstate\n" lines
                let mut len = [0u8; 4];
                while out.read_exact(&mut len).is_ok() {
                    let n = std::str::from_utf8(&len).ok().and_then(|h| usize::from_str_radix(h, 16).ok()).unwrap_or(0);
                    let mut body = vec![0u8; n];
                    if out.read_exact(&mut body).is_err() || !changed() {
                        let _ = child.kill();
                        return;
                    }
                }
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        if !changed() {
            return;
        }
        std::thread::sleep(Duration::from_secs(5));
    });
}

pub fn adb_device_present(adb_bin: &Path) -> bool {
    adb(adb_bin, &["get-state"]).map(|s| s.trim() == "device").unwrap_or(false)
}

/// Copy the bundled firmware to /tmp on the device and start it (until the next reboot).
pub fn start_firmware(adb_bin: &Path, firmware: &[u8]) -> Result<String, String> {
    let tmp = std::env::temp_dir().join("d200fw-upload");
    std::fs::write(&tmp, firmware).map_err(|e| e.to_string())?;
    let tmp_s = tmp.to_string_lossy().to_string();
    let push = adb(adb_bin, &["push", &tmp_s, "/tmp/d200fw"]);
    let _ = std::fs::remove_file(&tmp);
    push?;
    // adbd on the D200 kills background jobs unless they are detached with setsid.
    adb(adb_bin, &["shell", "chmod 755 /tmp/d200fw; setsid /tmp/d200fw </dev/null >/tmp/d200fw.log 2>&1 & sleep 2"])?;
    let net = adb(adb_bin, &["shell", "netstat -tln"])?;
    if net.contains(&format!(":{} ", port())) {
        Ok("d200fw is running on the device until it reboots.".into())
    } else {
        let log = adb(adb_bin, &["shell", "cat /tmp/d200fw.log"]).unwrap_or_default();
        Err(format!("d200fw did not start. Device log:\n{log}"))
    }
}

/// Replace an older d200fw on the device with `firmware` and restart it. A boot install
/// (/userdata/d200fw, see docs/firmware.md) is updated in place and its wrapper restarts
/// it; otherwise the copy in /tmp is replaced and started. The files are swapped with a
/// rename, which is safe while the old binary runs.
pub fn upgrade_firmware(adb_bin: &Path, firmware: &[u8]) -> Result<String, String> {
    let boot = adb(adb_bin, &["shell", "[ -x /userdata/d200fw/d200fw ] && [ ! -e /userdata/d200fw/disable ] && echo BOOT"])?.contains("BOOT");
    let dir = if boot { "/userdata/d200fw" } else { "/tmp" };
    let tmp = std::env::temp_dir().join("d200fw-upgrade");
    std::fs::write(&tmp, firmware).map_err(|e| e.to_string())?;
    let push = adb(adb_bin, &["push", &tmp.to_string_lossy(), &format!("{dir}/d200fw.new")]);
    let _ = std::fs::remove_file(&tmp);
    push?;
    // adb shell gives no exit codes: check the marker.
    let swap = adb(
        adb_bin,
        &["shell", &format!("chmod 755 {dir}/d200fw.new && mv -f {dir}/d200fw.new {dir}/d200fw && echo SWAPPED; kill $(pidof d200fw) 2>/dev/null; sleep 1")],
    )?;
    if !swap.contains("SWAPPED") {
        return Err(format!("could not replace {dir}/d200fw: {swap}"));
    }
    if boot {
        // the wrapper starts the new binary (after its short boot prompt)
        std::thread::sleep(Duration::from_secs(5));
        return Ok("d200fw updated (boot install).".into());
    }
    adb(adb_bin, &["shell", "setsid /tmp/d200fw </dev/null >/tmp/d200fw.log 2>&1 & sleep 2"])?;
    let net = adb(adb_bin, &["shell", "netstat -tln"])?;
    if net.contains(&format!(":{} ", port())) {
        Ok("d200fw updated; it runs until the device reboots.".into())
    } else {
        Err("the new d200fw did not start".into())
    }
}

// ------------------------------------------------------------------ device

struct Inner {
    w: TcpStream,
    rx: Receiver<String>,
}

pub struct Device {
    pub id: u64,
    pub version: String,
    /// Landscape (x, y, w, h) per key, as reported by the device.
    pub rects: Vec<[usize; 4]>,
    inner: Mutex<Inner>,
}

impl Device {
    /// Connect to d200fw. With `adb_bin` set, first forward the port over adb.
    pub fn connect(adb_bin: Option<&Path>, id: u64, events: Sender<DevEvent>) -> Result<Device, String> {
        let port = port();
        if let Some(adb_bin) = adb_bin {
            adb(adb_bin, &["forward", &format!("tcp:{port}"), &format!("tcp:{port}")])?;
        }
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let s = TcpStream::connect_timeout(&addr, Duration::from_secs(3)).map_err(|e| e.to_string())?;
        let _ = s.set_nodelay(true);
        let reader = s.try_clone().map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(reader).lines() {
                let Ok(line) = line else { break };
                let line = line.trim().to_string();
                if let Some(rest) = line.strip_prefix("KEY ") {
                    let mut it = rest.split_whitespace();
                    if let (Some(Ok(key)), Some(d)) = (it.next().map(str::parse), it.next()) {
                        let _ = events.send(DevEvent::Key { dev: id, key, down: d == "1" });
                    }
                } else if !line.is_empty() && tx.send(line).is_err() {
                    break;
                }
            }
            let _ = events.send(DevEvent::Closed { dev: id });
        });

        let mut dev = Device { id, version: String::new(), rects: vec![], inner: Mutex::new(Inner { w: s, rx }) };
        let pong = dev.cmd("PING", &[])?;
        dev.version = pong
            .strip_prefix("PONG d200fw ")
            .ok_or_else(|| format!("not a d200fw device: {pong}"))?
            .to_string();
        dev.rects = dev.layout()?;
        Ok(dev)
    }

    fn next_reply(inner: &Inner) -> Result<String, String> {
        match inner.rx.recv_timeout(REPLY_TIMEOUT) {
            Ok(r) => Ok(r),
            Err(RecvTimeoutError::Timeout) => Err("device did not answer".into()),
            Err(RecvTimeoutError::Disconnected) => Err("connection to device closed".into()),
        }
    }

    fn cmd(&self, line: &str, payload: &[u8]) -> Result<String, String> {
        let mut g = self.inner.lock().unwrap();
        let mut msg = Vec::with_capacity(line.len() + 1 + payload.len());
        msg.extend_from_slice(line.as_bytes());
        msg.push(b'\n');
        msg.extend_from_slice(payload);
        g.w.write_all(&msg).map_err(|e| e.to_string())?;
        let r = Self::next_reply(&g)?;
        if r.starts_with("ERR") {
            Err(r)
        } else {
            Ok(r)
        }
    }

    fn layout(&self) -> Result<Vec<[usize; 4]>, String> {
        let mut g = self.inner.lock().unwrap();
        g.w.write_all(b"LAYOUT\n").map_err(|e| e.to_string())?;
        let mut rects = vec![[0usize; 4]; crate::config::NUM_KEYS];
        loop {
            let r = Self::next_reply(&g)?;
            if r == "OK" {
                break;
            }
            let v: Vec<usize> = r.split_whitespace().skip(1).filter_map(|s| s.parse().ok()).collect();
            if let [i, x, y, w, h] = v[..] {
                if i < rects.len() {
                    rects[i] = [x, y, w, h];
                }
            }
        }
        Ok(rects)
    }

    /// RGB888 for a rectangle anywhere on the 1280x720 screen (landscape coordinates).
    pub fn blit(&self, x: usize, y: usize, w: usize, h: usize, rgb: &[u8]) -> Result<(), String> {
        self.cmd(&format!("BLIT {x} {y} {w} {h}"), rgb).map(|_| ())
    }

    pub fn image(&self, key: usize, rgb: &[u8]) -> Result<(), String> {
        self.cmd(&format!("IMG {key} {}", rgb.len()), rgb).map(|_| ())
    }

    /// Let the device draw the screensaver itself (d200fw 0.2+). Err on older firmware.
    pub fn saver(&self, scene: &str, fps: u32, motion: bool) -> Result<(), String> {
        self.cmd(&format!("SAVER {scene} {fps} {}", motion as u8), &[]).map(|_| ())
    }

    /// Show or hide the "notification waiting" dot of the device's screensaver.
    pub fn saver_dot(&self, on: bool) -> Result<(), String> {
        self.cmd(&format!("SAVER DOT {}", on as u8), &[]).map(|_| ())
    }

    pub fn saver_off(&self) -> Result<(), String> {
        self.cmd("SAVER OFF", &[]).map(|_| ())
    }

    pub fn clear(&self) -> Result<(), String> {
        self.cmd("CLEAR 000000", &[]).map(|_| ())
    }

    pub fn brightness(&self, v: u8) -> Result<(), String> {
        self.cmd(&format!("BRIGHT {v}"), &[]).map(|_| ())
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        if let Ok(g) = self.inner.lock() {
            let _ = g.w.shutdown(Shutdown::Both);
        }
    }
}
