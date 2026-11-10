//! D200 hardware: framebuffer, key layout, key input, backlight.
//!
//! Values come from measurements published by the D200-Unchained project
//! (docs/hardware.md) and the BadUStudio project.
//!
//! The panel is a 720x1280 portrait framebuffer (XRGB8888, stride 2880) mounted
//! rotated, so everything here works in *landscape* coordinates (1280x720,
//! origin top-left as you look at the device) and converts on write:
//!   fb_x = y,  fb_y = 1279 - x

use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::FileExt;

pub const SCREEN_W: usize = 1280; // landscape
pub const SCREEN_H: usize = 720;
const FB_W: usize = 720; // portrait, as the kernel sees it
const FB_STRIDE: usize = FB_W * 4;

pub const KEY_SIZE: usize = 196;
pub const NUM_KEYS: usize = 14;

/// Landscape window (x, y, w, h) of logical key `idx`.
/// Logical order is reading order: 0-4 top row, 5-9 middle row,
/// 10-12 bottom row, 13 the wide window bottom right.
pub fn key_rect(idx: usize) -> Option<(usize, usize, usize, usize)> {
    const X: [usize; 5] = [11, 271, 531, 791, 1051];
    const Y: [usize; 3] = [3, 262, 521];
    match idx {
        0..=9 => Some((X[idx % 5], Y[idx / 5], KEY_SIZE, KEY_SIZE)),
        10..=12 => Some((X[idx - 10], Y[2], KEY_SIZE, KEY_SIZE)),
        13 => Some((X[3], Y[2], 1247 - 791, KEY_SIZE)),
        _ => None,
    }
}

/// Linux keycode from the matrix keypad -> logical key index.
pub fn keycode_to_index(code: u16) -> Option<usize> {
    const MAP: [u16; NUM_KEYS] = [29, 15, 14, 13, 12, 11, 10, 9, 8, 7, 34, 33, 31, 30];
    MAP.iter().position(|&c| c == code)
}

pub struct Framebuffer {
    file: File,
}

impl Framebuffer {
    pub fn open(path: &str) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        Ok(Self { file })
    }

    /// Blit an RGB888 landscape image (`w` x `h`, row-major) at landscape (x, y).
    /// Clipped to the screen.
    pub fn blit(&self, x: usize, y: usize, w: usize, h: usize, rgb: &[u8]) -> io::Result<()> {
        if x >= SCREEN_W || y >= SCREEN_H || rgb.len() < w * h * 3 {
            return Ok(());
        }
        let cw = w.min(SCREEN_W - x);
        let ch = h.min(SCREEN_H - y);
        // One landscape column == one framebuffer row segment.
        let mut row = vec![0u8; ch * 4];
        for lx in 0..cw {
            for ly in 0..ch {
                let s = (ly * w + lx) * 3;
                let d = ly * 4;
                row[d] = rgb[s + 2]; // B
                row[d + 1] = rgb[s + 1]; // G
                row[d + 2] = rgb[s]; // R
                row[d + 3] = 0;
            }
            let fb_y = SCREEN_W - 1 - (x + lx);
            let off = fb_y * FB_STRIDE + y * 4;
            self.file.write_all_at(&row, off as u64)?;
        }
        Ok(())
    }

    /// Fill a landscape rectangle with one colour.
    pub fn fill(&self, x: usize, y: usize, w: usize, h: usize, rgb: [u8; 3]) -> io::Result<()> {
        if x >= SCREEN_W || y >= SCREEN_H {
            return Ok(());
        }
        let cw = w.min(SCREEN_W - x);
        let ch = h.min(SCREEN_H - y);
        let px = [rgb[2], rgb[1], rgb[0], 0];
        let row: Vec<u8> = px.iter().copied().cycle().take(ch * 4).collect();
        for lx in 0..cw {
            let fb_y = SCREEN_W - 1 - (x + lx);
            self.file
                .write_all_at(&row, (fb_y * FB_STRIDE + y * 4) as u64)?;
        }
        Ok(())
    }

    pub fn clear(&self, rgb: [u8; 3]) -> io::Result<()> {
        self.fill(0, 0, SCREEN_W, SCREEN_H, rgb)
    }
}

const BACKLIGHT: &str = "/sys/class/backlight/backlight/brightness";

pub fn set_backlight(v: u8) -> io::Result<()> {
    std::fs::write(BACKLIGHT, format!("{v}\n"))
}

pub fn get_backlight() -> Option<u8> {
    std::fs::read_to_string(BACKLIGHT).ok()?.trim().parse().ok()
}

/// Key press/release from the matrix keypad.
#[derive(Debug, Clone, Copy)]
pub struct KeyEvent {
    pub index: usize,
    pub pressed: bool,
}

/// Blocking reader for /dev/input/event0 (32-bit kernel: 16-byte events).
pub struct Keypad {
    file: File,
}

impl Keypad {
    pub fn open(path: &str) -> io::Result<Self> {
        Ok(Self { file: File::open(path)? })
    }

    /// Block until the next key event on a known key.
    pub fn next(&mut self) -> io::Result<KeyEvent> {
        let mut buf = [0u8; 16];
        loop {
            self.file.read_exact(&mut buf)?;
            let ty = u16::from_le_bytes([buf[8], buf[9]]);
            let code = u16::from_le_bytes([buf[10], buf[11]]);
            let value = i32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
            const EV_KEY: u16 = 1;
            if ty != EV_KEY || value == 2 {
                continue; // ignore sync/scan events and autorepeat
            }
            if let Some(index) = keycode_to_index(code) {
                return Ok(KeyEvent { index, pressed: value == 1 });
            }
        }
    }
}
