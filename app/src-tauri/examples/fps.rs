//! Throwaway measurement (Deckling work package 1): how many key images per second
//! reach the D200 over USB? Recolours all 14 keys in a loop and prints the rate.
//!
//!   cd app/src-tauri && cargo run --release --example fps -- [seconds]
//!
//! Needs a D200 over USB (adb). Starts the bundled d200fw if it is not running.
//! Quit D200 Deck first: only one client can hold the device.
//! Against the simulator: D200_DIRECT=1 D200_PORT=7399 cargo run --release --example fps
use d200_deck_lib::device::{self, Device};
use d200_deck_lib::render::key_size;
use std::time::{Duration, Instant};

fn main() {
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(10);
    let direct = std::env::var_os("D200_DIRECT").is_some();
    let adb = if direct { None } else { Some(device::adb_path().expect("adb not found")) };
    let (tx, _rx) = std::sync::mpsc::channel();
    let dev = match Device::connect(adb.as_deref(), 1, tx.clone()) {
        Ok(d) => d,
        Err(_) if !direct => {
            let adb = adb.as_deref().unwrap();
            println!("starting d200fw: {}", device::start_firmware(adb, d200_deck_lib::FIRMWARE).expect("start d200fw"));
            std::thread::sleep(Duration::from_secs(2));
            Device::connect(Some(adb), 1, tx).expect("connect to d200fw")
        }
        Err(e) => panic!("connect: {e}"),
    };
    println!("connected to d200fw {}", dev.version);

    // One frame per key, a different solid colour each round.
    let frames: Vec<Vec<u8>> = (0..14).map(|k| vec![0u8; (key_size(k).0 * key_size(k).1 * 3) as usize]).collect();
    let mut frames = frames;
    let (mut images, mut rounds) = (0u64, 0u64);
    let mut worst = Duration::ZERO;
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(secs) {
        let hue = (rounds * 23 % 360) as f32;
        let [r, g, b] = hsv(hue);
        let t0 = Instant::now();
        for (k, f) in frames.iter_mut().enumerate() {
            for px in f.chunks_exact_mut(3) {
                px.copy_from_slice(&[r, g, b]);
            }
            dev.image(k, f).expect("send image");
            images += 1;
        }
        worst = worst.max(t0.elapsed());
        rounds += 1;
    }
    let t = start.elapsed().as_secs_f64();
    let bytes: usize = frames.iter().map(Vec::len).sum();
    println!("{images} images in {t:.1} s");
    println!("{:.1} images/s, {:.1} full rounds (all 14 keys)/s", images as f64 / t, rounds as f64 / t);
    println!("slowest round {:.0} ms, {:.1} MB/s", worst.as_secs_f64() * 1000.0, (bytes as f64 * rounds as f64) / t / 1e6);
}

fn hsv(h: f32) -> [u8; 3] {
    let x = 1.0 - ((h / 60.0) % 2.0 - 1.0).abs();
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
}
