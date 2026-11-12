//! The screensaver, drawn on the device (`SAVER` command), so the host sends no frames.
//! The scene is the app's (`scene` crate); this file only runs it on the framebuffer.
//!
//! It fades in over what the keys showed (read back from the framebuffer), runs at the
//! host's frame rate and stops on any key press, any drawing command, `SAVER OFF`, or
//! when the host goes away. `SAVER DOT 1` shows a small mint dot on the wide key (a
//! notification is waiting).

use crate::hw::{self, Framebuffer, NUM_KEYS};
use scene::tiny_skia::Pixmap;
use scene::{Scene, Season};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const FADE: f32 = 2.2;
/// Key content fades out a little faster than the scene comes up.
const CONTENT_FADE: f32 = 1.9;
const MINT: [u8; 3] = [105, 224, 193];

struct Running {
    stop: Arc<AtomicBool>,
    dot: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

static SAVER: Mutex<Option<Running>> = Mutex::new(None);

pub fn running() -> bool {
    SAVER.lock().unwrap().is_some()
}

/// Stop the screensaver and wait until it no longer draws.
pub fn stop() {
    let r = SAVER.lock().unwrap().take();
    if let Some(r) = r {
        r.stop.store(true, Ordering::SeqCst);
        let _ = r.thread.join();
    }
}

pub fn set_dot(on: bool) {
    if let Some(r) = SAVER.lock().unwrap().as_ref() {
        r.dot.store(on, Ordering::SeqCst);
    }
}

pub fn start(fb: Arc<Framebuffer>, season: Season, fps: u32, motion: bool) {
    stop();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let dot = Arc::new(AtomicBool::new(false));
    let (s, d) = (stop_flag.clone(), dot.clone());
    let thread = std::thread::spawn(move || run(&fb, season, fps.clamp(1, 20), motion, &s, &d));
    *SAVER.lock().unwrap() = Some(Running { stop: stop_flag, dot, thread });
}

fn run(fb: &Framebuffer, season: Season, fps: u32, motion: bool, stop: &AtomicBool, dot: &AtomicBool) {
    let Some(mut pm) = Pixmap::new(scene::W as u32, scene::H as u32) else { return };
    let rects: Vec<_> = (0..NUM_KEYS).filter_map(hw::key_rect).collect();
    // what the keys show now, to fade from
    let before: Vec<Vec<u8>> = rects.iter().map(|&(x, y, w, h)| fb.read(x, y, w, h).unwrap_or_else(|_| vec![0; w * h * 3])).collect();
    let sc = Scene::new();
    let frame = Duration::from_millis(1000 / fps as u64);
    let since = Instant::now();
    let mut sent_still = false;
    while !stop.load(Ordering::SeqCst) {
        let t0 = Instant::now();
        let el = since.elapsed().as_secs_f32();
        let fade = (el / FADE).min(1.0);
        let pending = dot.load(Ordering::SeqCst);
        // A still scene only needs frames while fading in (and when the dot changes).
        if motion || fade < 1.0 || !sent_still {
            sc.render(season, if motion { el } else { 0.0 }, &mut pm);
            let data = pm.data();
            for (k, &(x, y, w, h)) in rects.iter().enumerate() {
                let mut rgb = Vec::with_capacity(w * h * 3);
                for row in y..y + h {
                    let o = (row * scene::W as usize + x) * 4;
                    for px in data[o..o + w * 4].chunks_exact(4) {
                        rgb.extend_from_slice(&px[..3]);
                    }
                }
                if k == NUM_KEYS - 1 && pending {
                    draw_dot(&mut rgb, w);
                }
                if fade < 1.0 {
                    let a = (fade * FADE / CONTENT_FADE).min(1.0);
                    for (o, f) in rgb.iter_mut().zip(before[k].iter()) {
                        *o = (*f as f32 * (1.0 - a) + *o as f32 * a) as u8;
                    }
                }
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let _ = fb.blit(x, y, w, h, &rgb);
            }
            sent_still = fade >= 1.0;
        }
        if !motion && fade >= 1.0 {
            // wait for a stop, or a change of the dot
            while !stop.load(Ordering::SeqCst) && dot.load(Ordering::SeqCst) == pending {
                std::thread::sleep(Duration::from_millis(100));
            }
            sent_still = false;
            continue;
        }
        if let Some(rest) = frame.checked_sub(t0.elapsed()) {
            std::thread::sleep(rest);
        }
    }
}

/// The quiet indicator for a waiting notification: a mint dot, top right of the wide key.
fn draw_dot(rgb: &mut [u8], w: usize) {
    for yy in 14..26 {
        for xx in (w - 26)..(w - 14) {
            let (dx, dy) = (xx as f32 - (w as f32 - 20.0), yy as f32 - 20.0);
            if dx * dx + dy * dy <= 36.0 {
                let o = (yy * w + xx) * 3;
                rgb[o..o + 3].copy_from_slice(&MINT);
            }
        }
    }
}
