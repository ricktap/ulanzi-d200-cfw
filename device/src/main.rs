//! d200fw: replacement UI for the Ulanzi D200 / D200H stream controller.
//!
//! Runs on the device in place of the stock `UlanziDeckKey` app. It owns the
//! screen and the keys and speaks a small line protocol on 127.0.0.1:7300
//! (reached from the computer with `adb forward`). The computer renders key
//! images and decides what key presses do; see PROTOCOL.md.

mod font;
mod hw;
mod rle;
mod saver;

use hw::{Framebuffer, KeyEvent, Keypad, KEY_SIZE, NUM_KEYS, SCREEN_H, SCREEN_W};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const VERSION: &str = env!("CARGO_PKG_VERSION");

struct Args {
    port: u16,
    fb: String,
    input: String,
    boot: bool,
    stock: Option<String>,
    stock_args: Vec<String>,
    kill_vendor: bool,
    test: bool,
    boot_wait: u64,
}

const USAGE: &str = "\
d200fw - replacement UI for the Ulanzi D200/D200H

usage: d200fw [options] [--stock <path> [-- <stock args>...]]
  --port <n>        TCP port on 127.0.0.1 (default 7300)
  --fb <dev>        framebuffer (default /dev/fb0)
  --input <dev>     keypad event device (default /dev/input/event0)
  --boot            boot mode: offer the stock UI first (needs --stock)
  --boot-wait <s>   seconds to wait for a key in boot mode (default 3)
  --stock <path>    stock app to exec when a key is held at boot
  --no-kill         do not stop the vendor app/watcher
  --test            show the key layout, print key presses, no server
";

fn parse_args() -> Args {
    let mut a = Args {
        port: 7300,
        fb: "/dev/fb0".into(),
        input: "/dev/input/event0".into(),
        boot: false,
        stock: None,
        stock_args: vec![],
        kill_vendor: true,
        test: false,
        boot_wait: 3,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = |name: &str| {
            it.next().unwrap_or_else(|| {
                eprintln!("missing value for {name}");
                std::process::exit(2)
            })
        };
        match arg.as_str() {
            "--port" => a.port = val("--port").parse().expect("bad port"),
            "--fb" => a.fb = val("--fb"),
            "--input" => a.input = val("--input"),
            "--boot" => a.boot = true,
            "--boot-wait" => a.boot_wait = val("--boot-wait").parse().expect("bad seconds"),
            "--stock" => a.stock = Some(val("--stock")),
            "--no-kill" => a.kill_vendor = false,
            "--test" => a.test = true,
            "--" => {
                a.stock_args = it.by_ref().collect();
                break;
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0)
            }
            other => {
                eprintln!("unknown option {other}\n{USAGE}");
                std::process::exit(2)
            }
        }
    }
    a
}

// ---------------------------------------------------------------- vendor app

/// PIDs whose executable name contains one of `names`.
fn find_procs(names: &[&str]) -> Vec<u32> {
    let me = std::process::id();
    let mut out = vec![];
    let Ok(dir) = std::fs::read_dir("/proc") else { return out };
    for e in dir.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else { continue };
        if pid == me {
            continue;
        }
        let Ok(cmd) = std::fs::read(format!("/proc/{pid}/cmdline")) else { continue };
        let argv0 = cmd.split(|&b| b == 0).next().unwrap_or(&[]);
        let argv0 = String::from_utf8_lossy(argv0);
        let base = argv0.rsplit('/').next().unwrap_or("");
        if names.iter().any(|n| base.contains(n)) {
            out.push(pid);
        }
    }
    out
}

/// Stop the stock app and its watchdog. They restart each other, so freeze
/// both first, then kill both.
fn stop_vendor() {
    let names = ["UlanziDeckKey", "WatcherProcess"];
    for attempt in 0..5 {
        let pids = find_procs(&names);
        if pids.is_empty() {
            if attempt > 0 {
                eprintln!("d200fw: vendor processes stopped");
            }
            return;
        }
        let list: Vec<String> = pids.iter().map(|p| p.to_string()).collect();
        eprintln!("d200fw: stopping vendor pids {}", list.join(" "));
        let _ = Command::new("kill").arg("-STOP").args(&list).status();
        let _ = Command::new("kill").arg("-KILL").args(&list).status();
        std::thread::sleep(Duration::from_millis(300));
    }
    eprintln!("d200fw: warning: vendor processes still running: {:?}", find_procs(&names));
}

// ---------------------------------------------------------------- drawing

fn render_key(fb: &Framebuffer, idx: usize, bg: [u8; 3], lines: &[(&str, usize, [u8; 3])]) {
    let Some((x, y, w, h)) = hw::key_rect(idx) else { return };
    let mut c = vec![0u8; w * h * 3];
    for px in c.chunks_exact_mut(3) {
        px.copy_from_slice(&bg);
    }
    let total: usize = lines.iter().map(|(_, s, _)| 7 * s).sum::<usize>()
        + lines.len().saturating_sub(1) * 12;
    let mut ty = h.saturating_sub(total) / 2;
    for (text, scale, rgb) in lines {
        font::draw_centered(&mut c, w, ty, text, *scale, *rgb);
        ty += 7 * scale + 12;
    }
    if let Err(e) = fb.blit(x, y, w, h, &c) {
        eprintln!("d200fw: draw failed: {e}");
    }
}

const GREY: [u8; 3] = [36, 38, 46];
const DIM: [u8; 3] = [150, 155, 170];
const WHITE: [u8; 3] = [255, 255, 255];

fn draw_layout(fb: &Framebuffer, title: &str, subtitle: &str) {
    let _ = fb.clear([0, 0, 0]);
    for i in 0..NUM_KEYS - 1 {
        render_key(fb, i, GREY, &[(&i.to_string(), 8, DIM)]);
    }
    render_key(fb, 13, GREY, &[(title, 6, WHITE), (subtitle, 3, DIM)]);
}

fn parse_rgb(s: &str) -> Option<[u8; 3]> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

// ---------------------------------------------------------------- server

/// The connected host, tagged with a generation so a replaced connection's
/// handler knows to stop.
type Client = Arc<Mutex<Option<(u64, TcpStream)>>>;

fn send(client: &Client, gen: u64, line: &str) {
    let mut g = client.lock().unwrap();
    if let Some((cg, s)) = g.as_mut() {
        if *cg == gen && s.write_all(line.as_bytes()).and_then(|_| s.write_all(b"\n")).is_err() {
            *g = None;
        }
    }
}

fn handle(stream: TcpStream, gen: u64, fb: &Arc<Framebuffer>, client: &Client) -> io::Result<()> {
    let mut rd = BufReader::with_capacity(64 * 1024, stream);
    let mut line = String::new();
    loop {
        line.clear();
        if (&mut rd).take(512).read_line(&mut line)? == 0 {
            return Ok(()); // host closed
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let Some(&cmd) = parts.first() else { continue };
        let num = |i: usize| -> Option<usize> { parts.get(i)?.parse().ok() };
        let cmd_uc = cmd.to_ascii_uppercase();
        // Anything that draws ends the screensaver first.
        if matches!(cmd_uc.as_str(), "IMG" | "IMGR" | "BLIT" | "BLITR" | "FILL" | "CLEAR") {
            saver::stop();
        }
        let reply: String = match cmd_uc.as_str() {
            "PING" => format!("PONG d200fw {VERSION}"),
            "INFO" => format!(
                "INFO version={VERSION} keys={NUM_KEYS} screen={SCREEN_W}x{SCREEN_H} key={KEY_SIZE}x{KEY_SIZE} brightness={}",
                hw::get_backlight().map(|b| b.to_string()).unwrap_or_else(|| "?".into())
            ),
            "LAYOUT" => {
                for i in 0..NUM_KEYS {
                    let (x, y, w, h) = hw::key_rect(i).unwrap();
                    send(client, gen, &format!("RECT {i} {x} {y} {w} {h}"));
                }
                "OK".into()
            }
            // IMG <key> <len> + len bytes RGB888, exactly the key's w*h*3
            "IMG" => match (num(1).and_then(hw::key_rect), num(2)) {
                (Some((x, y, w, h)), Some(len)) if len == w * h * 3 => {
                    let mut buf = vec![0u8; len];
                    rd.read_exact(&mut buf)?;
                    fb.blit(x, y, w, h, &buf).map(|_| "OK".into()).unwrap_or_else(|e| format!("ERR {e}"))
                }
                (Some((_, _, w, h)), Some(len)) => {
                    // Payload size is wrong: we cannot resync the stream, so drop the host.
                    send(client, gen, &format!("ERR IMG expects {} bytes, got {len}", w * h * 3));
                    return Ok(());
                }
                _ => "ERR usage: IMG <key 0-13> <len>".into(),
            },
            // BLIT <x> <y> <w> <h> + w*h*3 bytes RGB888, anywhere on the 1280x720 screen
            "BLIT" => match (num(1), num(2), num(3), num(4)) {
                (Some(x), Some(y), Some(w), Some(h)) if w > 0 && h > 0 && w <= SCREEN_W && h <= SCREEN_H => {
                    let mut buf = vec![0u8; w * h * 3];
                    rd.read_exact(&mut buf)?;
                    fb.blit(x, y, w, h, &buf).map(|_| "OK".into()).unwrap_or_else(|e| format!("ERR {e}"))
                }
                _ => {
                    send(client, gen, "ERR usage: BLIT <x> <y> <w 1-1280> <h 1-720>");
                    return Ok(());
                }
            },
            // IMGR <key> <len> / BLITR <x> <y> <w> <h> <len>: run-length coded RGB888 (rle.rs).
            // The payload length is given, so a bad image does not desync the stream.
            "IMGR" | "BLITR" => {
                let rect = if cmd_uc == "IMGR" {
                    num(1).and_then(hw::key_rect).map(|r| (r, num(2)))
                } else {
                    match (num(1), num(2), num(3), num(4)) {
                        (Some(x), Some(y), Some(w), Some(h)) if w > 0 && h > 0 && w <= SCREEN_W && h <= SCREEN_H => Some(((x, y, w, h), num(5))),
                        _ => None,
                    }
                };
                match rect {
                    Some(((x, y, w, h), Some(len))) if len <= w * h * 4 + 16 => {
                        let mut buf = vec![0u8; len];
                        rd.read_exact(&mut buf)?;
                        match rle::decode(&buf, w * h) {
                            Ok(rgb) => fb.blit(x, y, w, h, &rgb).map(|_| "OK".into()).unwrap_or_else(|e| format!("ERR {e}")),
                            Err(e) => format!("ERR {e}"),
                        }
                    }
                    _ => {
                        send(client, gen, "ERR usage: IMGR <key> <len> | BLITR <x> <y> <w> <h> <len>");
                        return Ok(());
                    }
                }
            }
            "FILL" => match (num(1).and_then(hw::key_rect), parts.get(2).and_then(|s| parse_rgb(s))) {
                (Some((x, y, w, h)), Some(rgb)) => {
                    fb.fill(x, y, w, h, rgb).map(|_| "OK".into()).unwrap_or_else(|e| format!("ERR {e}"))
                }
                _ => "ERR usage: FILL <key> <RRGGBB>".into(),
            },
            "CLEAR" => match parts.get(1).map_or(Some([0, 0, 0]), |s| parse_rgb(s)) {
                Some(rgb) => fb.clear(rgb).map(|_| "OK".into()).unwrap_or_else(|e| format!("ERR {e}")),
                None => "ERR usage: CLEAR [RRGGBB]".into(),
            },
            "BRIGHT" => match num(1) {
                Some(v) if v <= 255 => hw::set_backlight(v as u8)
                    .map(|_| "OK".into())
                    .unwrap_or_else(|e| format!("ERR {e}")),
                _ => "ERR usage: BRIGHT <0-255>".into(),
            },
            // SAVER <scene> <fps> <motion 0/1> | SAVER DOT <0/1> | SAVER OFF
            "SAVER" => match parts.get(1).map(|s| s.to_ascii_lowercase()).as_deref() {
                Some("off") => {
                    saver::stop();
                    "OK".into()
                }
                Some("dot") => {
                    saver::set_dot(parts.get(2) == Some(&"1"));
                    "OK".into()
                }
                Some(name) => match (scene::Season::from_name(name), num(2)) {
                    (Some(season), Some(fps)) => {
                        saver::start(fb.clone(), season, fps as u32, parts.get(3) != Some(&"0"));
                        "OK".into()
                    }
                    _ => "ERR usage: SAVER <snow|pink|beach|leaves|rain> <fps> <motion 0/1> | SAVER DOT <0/1> | SAVER OFF".into(),
                },
                None => "ERR usage: SAVER <scene> <fps> <motion> | SAVER DOT <0/1> | SAVER OFF".into(),
            },
            "BYE" => {
                send(client, gen, "OK");
                return Ok(());
            }
            _ => format!("ERR unknown command {cmd}"),
        };
        send(client, gen, &reply);
    }
}

fn serve(listener: TcpListener, fb: Arc<Framebuffer>, keys: Receiver<KeyEvent>) {
    let client: Client = Arc::new(Mutex::new(None));
    let next_gen = Arc::new(AtomicU64::new(1));

    // Key events -> whichever host is connected.
    {
        let client = client.clone();
        let fb = fb.clone();
        std::thread::spawn(move || {
            for ev in keys {
                if ev.pressed && saver::running() {
                    saver::stop(); // the host redraws the keys
                }
                let mut g = client.lock().unwrap();
                match g.as_mut() {
                    Some((_, s)) => {
                        let msg = format!("KEY {} {}\n", ev.index, ev.pressed as u8);
                        if s.write_all(msg.as_bytes()).is_err() {
                            *g = None;
                        }
                    }
                    None => {
                        // No host: give visual feedback so the device does not feel dead.
                        let bg = if ev.pressed { [0, 120, 255] } else { GREY };
                        if ev.index < 13 {
                            render_key(&fb, ev.index, bg, &[(&ev.index.to_string(), 8, DIM)]);
                        }
                    }
                }
            }
        });
    }

    draw_layout(&fb, "D200FW", "WAITING FOR HOST");
    eprintln!("d200fw {VERSION}: listening on {}", listener.local_addr().unwrap());

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let _ = stream.set_nodelay(true);
        let gen = next_gen.fetch_add(1, Ordering::SeqCst);
        let Ok(writer) = stream.try_clone() else { continue };
        if let Some((_, old)) = client.lock().unwrap().replace((gen, writer)) {
            let _ = old.shutdown(Shutdown::Both); // newest host wins
        }
        eprintln!("d200fw: host connected (#{gen})");
        let (fb, client) = (fb.clone(), client.clone());
        std::thread::spawn(move || {
            if let Err(e) = handle(stream, gen, &fb, &client) {
                eprintln!("d200fw: host #{gen}: {e}");
            }
            let mut g = client.lock().unwrap();
            if matches!(g.as_ref(), Some((cg, _)) if *cg == gen) {
                *g = None;
                drop(g);
                saver::stop();
                eprintln!("d200fw: host disconnected (#{gen})");
                draw_layout(&fb, "D200FW", "WAITING FOR HOST");
            }
        });
    }
}

// ---------------------------------------------------------------- main

fn main() {
    let args = parse_args();

    // Bind first: this doubles as a single-instance lock.
    let listener = if args.test {
        None
    } else {
        match TcpListener::bind(("127.0.0.1", args.port)) {
            Ok(l) => Some(l),
            Err(e) => {
                eprintln!("d200fw: port {} busy ({e}), another instance is running", args.port);
                std::process::exit(0);
            }
        }
    };

    if args.kill_vendor {
        stop_vendor();
        // The vendor watchdog looks for this file; keep it pointing at us (device only).
        let _ = std::fs::write("/tmp/app.pid", format!("{}\n", std::process::id()));
    }

    let fb = Arc::new(Framebuffer::open(&args.fb).unwrap_or_else(|e| {
        eprintln!("d200fw: cannot open {}: {e}", args.fb);
        std::process::exit(1)
    }));
    let mut keypad = Keypad::open(&args.input).unwrap_or_else(|e| {
        eprintln!("d200fw: cannot open {}: {e}", args.input);
        std::process::exit(1)
    });
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || loop {
        match keypad.next() {
            Ok(ev) => {
                if tx.send(ev).is_err() {
                    return;
                }
            }
            Err(e) => {
                eprintln!("d200fw: keypad: {e}");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    });

    if args.test {
        draw_layout(&fb, "TEST", "PRESS KEYS");
        println!("d200fw test mode: press keys, Ctrl+C to quit");
        for ev in rx {
            println!("key {} {}", ev.index, if ev.pressed { "down" } else { "up" });
            let bg = if ev.pressed { [0, 160, 80] } else { GREY };
            render_key(&fb, ev.index, bg, &[(&ev.index.to_string(), 8, WHITE)]);
        }
        return;
    }

    if args.boot {
        if let Some(stock) = &args.stock {
            draw_layout(&fb, "D200FW", "PRESS A KEY: STOCK UI");
            let deadline = Instant::now() + Duration::from_secs(args.boot_wait);
            loop {
                let left = deadline.saturating_duration_since(Instant::now());
                match rx.recv_timeout(left) {
                    Ok(ev) if ev.pressed => {
                        eprintln!("d200fw: key at boot, starting stock UI {stock}");
                        let _ = fb.clear([0, 0, 0]);
                        drop(listener);
                        let err = Command::new(stock).args(&args.stock_args).exec();
                        eprintln!("d200fw: exec {stock} failed: {err}");
                        std::process::exit(1);
                    }
                    Ok(_) => continue,
                    Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        }
    }

    serve(listener.unwrap(), fb, rx);
}
