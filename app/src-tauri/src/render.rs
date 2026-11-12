//! Drawing: a small canvas (tiny-skia) with text that falls back to the system
//! colour-emoji font, so emoji work as icons. Key faces are produced as RGB888.

use crate::config::{resolve, Look};
use ab_glyph::{point, Font, FontVec, GlyphId, PxScale, ScaleFont, VariableFont};
use image::{imageops, DynamicImage};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform};

pub fn key_size(key: usize) -> (u32, u32) {
    if key == crate::config::STATUS_KEY {
        (456, 196)
    } else {
        (196, 196)
    }
}

pub type Rgb = [u8; 3];

pub fn parse_color(s: &str) -> Option<Rgb> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(h) = s.strip_prefix('#') {
        let v = u32::from_str_radix(h, 16).ok()?;
        return match h.len() {
            6 => Some([(v >> 16) as u8, (v >> 8) as u8, v as u8]),
            3 => Some([((v >> 8) & 15) as u8 * 17, ((v >> 4) & 15) as u8 * 17, (v & 15) as u8 * 17]),
            _ => None,
        };
    }
    Some(match s.as_str() {
        "black" => [0, 0, 0],
        "white" => [255, 255, 255],
        "red" => [220, 38, 38],
        "green" => [22, 163, 74],
        "blue" => [37, 99, 235],
        "gray" | "grey" => [107, 114, 128],
        "orange" => [234, 88, 12],
        "yellow" => [234, 179, 8],
        "purple" => [124, 58, 237],
        "pink" => [219, 39, 119],
        "teal" => [13, 148, 136],
        _ => return None,
    })
}

pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 * (1.0 - t) + b[i] as f32 * t).round() as u8)
}

// ------------------------------------------------------------------ fonts

const MAIN_FONTS: &[&str] = &[
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "C:\\Windows\\Fonts\\segoeui.ttf",
    "C:\\Windows\\Fonts\\arial.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
];
const EMOJI_FONTS: &[&str] = &[
    "/System/Library/Fonts/Apple Color Emoji.ttc",
    "C:\\Windows\\Fonts\\seguiemj.ttf",
    "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
    "/usr/share/fonts/noto/NotoColorEmoji.ttf",
    "/usr/share/fonts/google-noto-emoji/NotoColorEmoji.ttf",
];
/// Symbol fonts for arrows and dingbats the main font lacks.
const SYMBOL_FONTS: &[&str] = &[
    "/System/Library/Fonts/Apple Symbols.ttf",
    "C:\\Windows\\Fonts\\seguisym.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
];

fn first_font(paths: &[&str]) -> Option<FontVec> {
    paths.iter().find_map(|p| std::fs::read(p).ok().and_then(|d| FontVec::try_from_vec_and_index(d, 0).ok()))
}

/// Font weight. The system UI font is variable, so all three come from one file.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Weight {
    Regular,
    Semi,
    Bold,
}

pub struct Fonts {
    main: FontVec,
    semi: FontVec,
    bold: FontVec,
    emoji: Option<FontVec>,
    symbols: Option<FontVec>,
    raster: Mutex<HashMap<(u16, u16), Option<Arc<Pixmap>>>>,
}

impl Fonts {
    pub fn load() -> Result<Fonts, String> {
        let weighted = |w: f32| {
            first_font(MAIN_FONTS).map(|mut f| {
                f.set_variation(b"wght", w);
                f
            })
        };
        Ok(Fonts {
            main: weighted(430.0).ok_or("no usable system font found")?,
            semi: weighted(620.0).ok_or("no usable system font found")?,
            bold: weighted(760.0).ok_or("no usable system font found")?,
            emoji: first_font(EMOJI_FONTS),
            symbols: first_font(SYMBOL_FONTS),
            raster: Mutex::new(HashMap::new()),
        })
    }

    /// Emoji are preferred from the colour font; everything else from the main font.
    fn pick(&self, c: char) -> Option<(Which, GlyphId)> {
        let has = |f: &FontVec| {
            let id = f.glyph_id(c);
            (id.0 != 0).then_some(id)
        };
        if is_emoji(c) {
            if let Some(id) = self.emoji.as_ref().and_then(has) {
                return Some((Which::Emoji, id));
            }
        }
        if let Some(id) = has(&self.main) {
            return Some((Which::Main, id));
        }
        if let Some(id) = self.symbols.as_ref().and_then(has) {
            return Some((Which::Symbols, id));
        }
        if let Some(id) = self.emoji.as_ref().and_then(has) {
            return Some((Which::Emoji, id));
        }
        None
    }

    fn font_w(&self, w: Which, wt: Weight) -> &FontVec {
        match (w, wt) {
            (Which::Main, Weight::Semi) => &self.semi,
            (Which::Main, Weight::Bold) => &self.bold,
            _ => self.font(w),
        }
    }

    fn font(&self, w: Which) -> &FontVec {
        match w {
            Which::Main => &self.main,
            Which::Emoji => self.emoji.as_ref().unwrap_or(&self.main),
            Which::Symbols => self.symbols.as_ref().unwrap_or(&self.main),
        }
    }

    /// Colour bitmap of an emoji glyph, cached.
    fn raster(&self, id: GlyphId, size: u16) -> Option<Arc<Pixmap>> {
        let emoji = self.emoji.as_ref()?;
        let key = (id.0, size);
        if let Some(v) = self.raster.lock().unwrap().get(&key) {
            return v.clone();
        }
        let pm = emoji.glyph_raster_image2(id, size).and_then(|g| {
            let img = image::load_from_memory(g.data).ok()?;
            pixmap_from_image(&img)
        });
        let pm = pm.map(Arc::new);
        self.raster.lock().unwrap().insert(key, pm.clone());
        pm
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Which {
    Main,
    Emoji,
    Symbols,
}

fn is_emoji(c: char) -> bool {
    let u = c as u32;
    (0x1F000..=0x1FAFF).contains(&u) || (0x2600..=0x27BF).contains(&u) || (0x2B00..=0x2BFF).contains(&u) || (0x2300..=0x23FF).contains(&u)
}

fn skip(c: char) -> bool {
    matches!(c, '\u{FE0F}' | '\u{FE0E}' | '\u{200D}' | '\u{20E3}')
}

/// True if `s` looks like an image path rather than an emoji / text icon.
pub fn is_image_path(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp"].iter().any(|e| l.ends_with(e))
}

// ------------------------------------------------------------------ canvas

pub struct Canvas {
    pub pm: Pixmap,
}

fn paint(c: Rgb, alpha: f32) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], (alpha.clamp(0.0, 1.0) * 255.0) as u8);
    p.anti_alias = true;
    p
}

fn pixmap_from_image(img: &DynamicImage) -> Option<Pixmap> {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut data = rgba.into_raw();
    for px in data.chunks_exact_mut(4) {
        let a = px[3] as u16;
        for c in &mut px[..3] {
            *c = ((*c as u16 * a + 127) / 255) as u8;
        }
    }
    Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(w, h)?)
}

impl Canvas {
    pub fn new(w: u32, h: u32, bg: Rgb) -> Canvas {
        let mut pm = Pixmap::new(w, h).expect("canvas size");
        pm.fill(tiny_skia::Color::from_rgba8(bg[0], bg[1], bg[2], 255));
        Canvas { pm }
    }
    pub fn w(&self) -> f32 {
        self.pm.width() as f32
    }
    pub fn h(&self) -> f32 {
        self.pm.height() as f32
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgb, alpha: f32) {
        let Some(path) = rounded_rect(x, y, w, h, r) else { return };
        self.pm.fill_path(&path, &paint(c, alpha), FillRule::Winding, Transform::identity(), None);
    }

    /// Progress ring from 12 o'clock, clockwise.
    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, width: f32, frac: f32, c: Rgb, track: Rgb) {
        let stroke = Stroke { width, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
        if let Some(p) = arc(cx, cy, r, 0.0, 1.0) {
            self.pm.stroke_path(&p, &paint(track, 1.0), &stroke, Transform::identity(), None);
        }
        if frac > 0.002 {
            if let Some(p) = arc(cx, cy, r, 0.0, frac.min(1.0)) {
                self.pm.stroke_path(&p, &paint(c, 1.0), &stroke, Transform::identity(), None);
            }
        }
    }

    /// Area graph of `values` (0..=max) inside the box.
    pub fn sparkline(&mut self, x: f32, y: f32, w: f32, h: f32, values: &[f32], max: f32, c: Rgb) {
        if values.len() < 2 || max <= 0.0 {
            return;
        }
        let n = values.len();
        let pt = |i: usize| {
            let v = (values[i] / max).clamp(0.0, 1.0);
            (x + w * i as f32 / (n - 1) as f32, y + h - v * h)
        };
        let mut line = PathBuilder::new();
        let mut area = PathBuilder::new();
        let (x0, y0) = pt(0);
        line.move_to(x0, y0);
        area.move_to(x, y + h);
        area.line_to(x0, y0);
        for i in 1..n {
            let (px, py) = pt(i);
            line.line_to(px, py);
            area.line_to(px, py);
        }
        area.line_to(x + w, y + h);
        area.close();
        if let Some(p) = area.finish() {
            self.pm.fill_path(&p, &paint(c, 0.16), FillRule::Winding, Transform::identity(), None);
        }
        if let Some(p) = line.finish() {
            let stroke = Stroke { width: 3.0, line_join: tiny_skia::LineJoin::Round, ..Default::default() };
            self.pm.stroke_path(&p, &paint(c, 1.0), &stroke, Transform::identity(), None);
        }
    }

    pub fn draw_pixmap(&mut self, src: &Pixmap, x: f32, y: f32, w: f32, h: f32) {
        let sx = w / src.width() as f32;
        let sy = h / src.height() as f32;
        let paint = PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() };
        self.pm.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::from_row(sx, 0.0, 0.0, sy, x, y), None);
    }

    /// Image scaled to cover the whole canvas.
    pub fn image_cover(&mut self, img: &DynamicImage) {
        let img = img.resize_to_fill(self.pm.width(), self.pm.height(), imageops::FilterType::Lanczos3);
        if let Some(p) = pixmap_from_image(&img) {
            self.pm.draw_pixmap(0, 0, p.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
        }
    }

    /// Draw a registry icon centred in the box; false if there is no such icon.
    pub fn set_icon(&mut self, name: &str, x: f32, y: f32, w: f32, h: f32, overrides: &[(&str, &str)]) -> bool {
        self.set_icon_env(name, x, y, w, h, overrides, None)
    }

    /// Same, resolving the icon's bindings in `env` (without one, the fallbacks draw).
    pub fn set_icon_env(&mut self, name: &str, x: f32, y: f32, w: f32, h: f32, overrides: &[(&str, &str)], env: Option<&IconEnv>) -> bool {
        let reg = crate::registry::current();
        let Some(def) = reg.icon(name) else { return false };
        // a key gave this state of its widget another icon (IconSwap)
        let def = ICON_SWAP.with(|s| s.borrow().iter().find(|(from, _)| *from == def.id).and_then(|(_, to)| reg.icon(to).cloned()));
        let def = match &def {
            Some(d) => d,
            None => reg.icon(name).expect("looked up above"),
        };
        let s = w.min(h).max(1.0);
        let empty = IconEnv::empty();
        let env = env.unwrap_or(&empty);
        let pm = crate::icons::render_bound(def, s as u32, overrides, &env.ctx, &mut env.reads.borrow_mut());
        if let Some(pm) = pm {
            let px = x + (w - s) / 2.0;
            let py = y + (h - s) / 2.0;
            self.pm.draw_pixmap(px as i32, py as i32, pm.as_ref().as_ref(), &PixmapPaint::default(), Transform::identity(), None);
        }
        true
    }

    /// Image scaled to cover the box (centre-cropped).
    pub fn image_cover_at(&mut self, img: &DynamicImage, x: f32, y: f32, w: f32, h: f32) {
        let img = img.resize_to_fill(w.max(1.0) as u32, h.max(1.0) as u32, imageops::FilterType::Lanczos3);
        if let Some(p) = pixmap_from_image(&img) {
            self.pm.draw_pixmap(x as i32, y as i32, p.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
        }
    }

    /// Text cut with an ellipsis to fit `max` pixels.
    pub fn fit(&self, f: &Fonts, text: &str, size: f32, max: f32) -> String {
        if self.text_width(f, text, size) <= max {
            return text.to_string();
        }
        let mut t: Vec<char> = text.chars().collect();
        while !t.is_empty() {
            t.pop();
            let s: String = t.iter().collect::<String>().trim_end().to_string() + "…";
            if self.text_width(f, &s, size) <= max {
                return s;
            }
        }
        "…".into()
    }

    /// Image scaled to fit the box, centred.
    pub fn image_fit(&mut self, img: &DynamicImage, x: f32, y: f32, w: f32, h: f32) {
        let s = (w / img.width() as f32).min(h / img.height() as f32);
        let (iw, ih) = (img.width() as f32 * s, img.height() as f32 * s);
        let img = img.resize(iw.max(1.0) as u32, ih.max(1.0) as u32, imageops::FilterType::Lanczos3);
        if let Some(p) = pixmap_from_image(&img) {
            let px = x + (w - p.width() as f32) / 2.0;
            let py = y + (h - p.height() as f32) / 2.0;
            self.pm.draw_pixmap(px as i32, py as i32, p.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
        }
    }

    /// Wash the whole face with a colour (used for the pressed look).
    pub fn overlay(&mut self, c: Rgb, alpha: f32) {
        let (w, h) = (self.w(), self.h());
        self.rect(0.0, 0.0, w, h, 0.0, c, alpha);
    }

    // -------------------------------------------------------------- text

    pub fn text_width(&self, f: &Fonts, text: &str, size: f32) -> f32 {
        self.tw(f, text, size, Weight::Regular, 0.0)
    }

    /// Width with weight and letter-spacing (`track` px between characters).
    pub fn tw(&self, f: &Fonts, text: &str, size: f32, wt: Weight, track: f32) -> f32 {
        let mut w = 0.0;
        let mut n = 0;
        let mut prev: Option<(Which, GlyphId)> = None;
        for c in text.chars().filter(|&c| !skip(c)) {
            let Some((which, id)) = f.pick(c) else { continue };
            n += 1;
            let sf = f.font_w(which, wt).as_scaled(PxScale::from(size));
            if let Some((pw, pid)) = prev {
                if pw == which {
                    w += sf.kern(pid, id);
                }
            }
            w += if which == Which::Emoji { size * 0.95 } else { sf.h_advance(id) };
            prev = Some((which, id));
        }
        w + track * (n.max(1) - 1) as f32
    }

    /// One line of text with its left edge at `x` and baseline at `baseline`.
    pub fn text_line(&mut self, f: &Fonts, text: &str, size: f32, x: f32, baseline: f32, fg: Rgb) {
        self.tl(f, text, size, x, baseline, fg, Weight::Regular, 0.0);
    }

    /// Text line with weight and letter-spacing.
    pub fn tl(&mut self, f: &Fonts, text: &str, size: f32, x: f32, baseline: f32, fg: Rgb, wt: Weight, track: f32) {
        let scale = PxScale::from(size);
        let mut x = x;
        let mut prev: Option<(Which, GlyphId)> = None;
        for c in text.chars().filter(|&c| !skip(c)) {
            let Some((which, id)) = f.pick(c) else { continue };
            let font = f.font_w(which, wt);
            let sf = font.as_scaled(scale);
            if let Some((pw, pid)) = prev {
                if pw == which {
                    x += sf.kern(pid, id);
                }
            }
            prev = Some((which, id));
            if which == Which::Emoji {
                let e = size * 0.88;
                if let Some(pm) = f.raster(id, (size * 1.5) as u16) {
                    self.draw_pixmap(&pm, x + size * 0.035, baseline - size * 0.76, e, e);
                    x += size * 0.95 + track;
                    continue;
                }
            }
            let advance = if which == Which::Emoji { size * 0.95 } else { sf.h_advance(id) };
            if let Some(og) = font.outline_glyph(id.with_scale_and_position(scale, point(x, baseline))) {
                let b = og.px_bounds();
                let (w, h) = (self.pm.width() as i32, self.pm.height() as i32);
                let data = self.pm.data_mut();
                og.draw(|gx, gy, cov| {
                    let px = b.min.x as i32 + gx as i32;
                    let py = b.min.y as i32 + gy as i32;
                    if px < 0 || py < 0 || px >= w || py >= h {
                        return;
                    }
                    let o = ((py * w + px) * 4) as usize;
                    let a = cov.clamp(0.0, 1.0);
                    for i in 0..3 {
                        data[o + i] = (data[o + i] as f32 * (1.0 - a) + fg[i] as f32 * a).round() as u8;
                    }
                });
            }
            x += advance + track;
        }
    }

    pub fn ascent(&self, f: &Fonts, size: f32) -> f32 {
        f.main.as_scaled(PxScale::from(size)).ascent()
    }

    /// Centred text in the band `top..bottom`, multi-line, shrunk until it fits
    /// (never below `min`). Returns the size used.
    pub fn text_block(&mut self, f: &Fonts, text: &str, size: f32, min: f32, top: f32, bottom: f32, fg: Rgb) -> f32 {
        let lines: Vec<&str> = text.split('\n').collect();
        let w = self.w();
        let spacing = 0.12;
        let fits = |c: &Canvas, s: f32| {
            let tw = lines.iter().map(|l| c.text_width(f, l, s)).fold(0.0, f32::max);
            let th = s * lines.len() as f32 * (1.0 + spacing);
            tw <= w - 14.0 && th <= bottom - top - 6.0
        };
        let mut s = size;
        while s > min && !fits(self, s) {
            s -= 2.0;
        }
        let lh = s * (1.0 + spacing);
        let th = lh * lines.len() as f32 - s * spacing;
        let asc = self.ascent(f, s);
        let mut y = top + ((bottom - top) - th) / 2.0;
        for l in lines {
            let lw = self.text_width(f, l, s);
            self.text_line(f, l, s, (w - lw) / 2.0, y + asc * 0.92, fg);
            y += lh;
        }
        s
    }

    /// Text centred horizontally on `cx`.
    pub fn text_centered(&mut self, f: &Fonts, text: &str, size: f32, cx: f32, baseline: f32, fg: Rgb) {
        let w = self.text_width(f, text, size);
        self.text_line(f, text, size, cx - w / 2.0, baseline, fg);
    }

    /// An icon: a Night Signal icon name, an image file, or emoji/text, fitted into the box.
    /// Set icons keep their own palette; a non-white `fg` recolours their ivory base strokes.
    pub fn icon(&mut self, f: &Fonts, base: &Path, icon: &str, x: f32, y: f32, w: f32, h: f32, fg: Rgb) {
        self.icon_s(f, base, icon, None, x, y, w, h, fg)
    }

    /// `icon` with bindings resolved in `env`.
    pub fn icon_s(&mut self, f: &Fonts, base: &Path, icon: &str, env: Option<&IconEnv>, x: f32, y: f32, w: f32, h: f32, fg: Rgb) {
        let hex = format!("#{:02X}{:02X}{:02X}", fg[0], fg[1], fg[2]);
        let ovr: Vec<(&str, &str)> = if fg == [255, 255, 255] || fg == IVORY { vec![] } else { vec![("icon-base", hex.as_str())] };
        if self.set_icon_env(icon, x, y, w, h, &ovr, env) {
            return;
        }
        if is_image_path(icon) {
            if let Ok(img) = image::open(resolve(base, icon)) {
                self.image_fit(&img, x, y, w, h);
            }
            return;
        }
        let size = h.min(w);
        let tw = self.text_width(f, icon, size);
        let s = if tw > w { size * w / tw } else { size };
        let tw = self.text_width(f, icon, s);
        let asc = self.ascent(f, s);
        self.text_line(f, icon, s, x + (w - tw) / 2.0, y + (h - s) / 2.0 + asc * 0.95, fg);
    }

    // -------------------------------------------------------------- design primitives

    /// Uppercase, letter-spaced label centred on `cx`, shrunk to fit `max` px.
    pub fn caps(&mut self, f: &Fonts, text: &str, size: f32, cx: f32, baseline: f32, fg: Rgb, max: f32) {
        let t = text.to_uppercase();
        let mut s = size;
        let track = |s: f32| s * 0.14;
        while s > 11.0 && self.tw(f, &t, s, Weight::Semi, track(s)) > max {
            s -= 1.0;
        }
        let w = self.tw(f, &t, s, Weight::Semi, track(s));
        self.tl(f, &t, s, cx - w / 2.0, baseline, fg, Weight::Semi, track(s));
    }

    /// Uppercase label starting at `x` (no shrinking).
    pub fn caps_at(&mut self, f: &Fonts, text: &str, size: f32, x: f32, baseline: f32, fg: Rgb) -> f32 {
        let t = text.to_uppercase();
        let w = self.tw(f, &t, size, Weight::Semi, size * 0.14);
        self.tl(f, &t, size, x, baseline, fg, Weight::Semi, size * 0.14);
        w
    }

    /// Big bold figure ("79%", "24:58") centred on `cx`, shrunk to fit `max`.
    pub fn figure(&mut self, f: &Fonts, text: &str, size: f32, cx: f32, baseline: f32, fg: Rgb, max: f32) {
        let mut s = size;
        while s > 14.0 && self.tw(f, text, s, Weight::Bold, -s * 0.02) > max {
            s -= 2.0;
        }
        let w = self.tw(f, text, s, Weight::Bold, -s * 0.02);
        self.tl(f, text, s, cx - w / 2.0, baseline, fg, Weight::Bold, -s * 0.02);
    }

    /// Big bold figure with its left edge at `x`.
    pub fn figure_left(&mut self, f: &Fonts, text: &str, size: f32, x: f32, baseline: f32, fg: Rgb, max: f32) {
        let mut s = size;
        while s > 14.0 && self.tw(f, text, s, Weight::Bold, -s * 0.02) > max {
            s -= 2.0;
        }
        self.tl(f, text, s, x - s * 0.03, baseline, fg, Weight::Bold, -s * 0.02);
    }

    /// Thin line graph of `values` (0..=max), no fill.
    pub fn line_graph(&mut self, x: f32, y: f32, w: f32, h: f32, values: &[f32], max: f32, c: Rgb) {
        if values.len() < 2 || max <= 0.0 {
            return;
        }
        let n = values.len();
        let mut pb = PathBuilder::new();
        for (i, v) in values.iter().enumerate() {
            let px = x + w * i as f32 / (n - 1) as f32;
            let py = y + h - (v / max).clamp(0.0, 1.0) * h;
            if i == 0 { pb.move_to(px, py) } else { pb.line_to(px, py) }
        }
        if let Some(p) = pb.finish() {
            let stroke = Stroke { width: 2.6, line_join: tiny_skia::LineJoin::Round, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
            self.pm.stroke_path(&p, &paint(c, 1.0), &stroke, Transform::identity(), None);
        }
    }

    /// Soft directional light from the upper left, tinted `c` (wide-card state emphasis).
    pub fn glow(&mut self, c: Rgb, strength: f32) {
        let (w, h) = (self.w(), self.h());
        let stops = vec![
            tiny_skia::GradientStop::new(0.0, tiny_skia::Color::from_rgba8(c[0], c[1], c[2], (strength.clamp(0.0, 1.0) * 255.0) as u8)),
            tiny_skia::GradientStop::new(1.0, tiny_skia::Color::from_rgba8(c[0], c[1], c[2], 0)),
        ];
        if let Some(shader) = tiny_skia::RadialGradient::new(
            tiny_skia::Point::from_xy(0.0, 0.0),
            tiny_skia::Point::from_xy(0.0, 0.0),
            w.max(h) * 0.85,
            stops,
            tiny_skia::SpreadMode::Pad,
            Transform::identity(),
        ) {
            let mut p = Paint::default();
            p.shader = shader;
            p.anti_alias = true;
            if let Some(r) = Rect::from_xywh(0.0, 0.0, w, h) {
                self.pm.fill_rect(r, &p, Transform::identity(), None);
            }
        }
    }

    /// Filled status pill ("ON", "MUTED") centred on (cx, cy).
    pub fn pill(&mut self, f: &Fonts, text: &str, cx: f32, cy: f32, color: Rgb) {
        let t = text.to_uppercase();
        let size = 17.0;
        let tw = self.tw(f, &t, size, Weight::Bold, 2.4);
        let (w, h) = (tw + 34.0, 32.0);
        self.rect(cx - w / 2.0, cy - h / 2.0, w, h, h / 2.0, color, 1.0);
        self.tl(f, &t, size, cx - tw / 2.0, cy + 6.5, BG, Weight::Bold, 2.4);
    }

    /// Hairline.
    pub fn hline(&mut self, x1: f32, x2: f32, y: f32, c: Rgb) {
        self.rect(x1, y, x2 - x1, 2.0, 0.0, c, 1.0);
    }

    pub fn vline(&mut self, x: f32, y1: f32, y2: f32, c: Rgb) {
        self.rect(x, y1, 2.0, y2 - y1, 0.0, c, 1.0);
    }

    /// Outlined rounded rectangle.
    pub fn stroke_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, width: f32, c: Rgb) {
        let Some(path) = rounded_rect(x + width / 2.0, y + width / 2.0, w - width, h - width, r) else { return };
        let stroke = Stroke { width, ..Default::default() };
        self.pm.stroke_path(&path, &paint(c, 1.0), &stroke, Transform::identity(), None);
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, width: f32, c: Rgb) {
        self.ring(cx, cy, r, width, 1.0, c, c);
    }

    pub fn into_rgb(self) -> Vec<u8> {
        // Opaque canvas: premultiplied == straight.
        self.pm.data().chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect()
    }
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    if r <= 0.0 {
        return Some(PathBuilder::from_rect(Rect::from_xywh(x, y, w, h)?));
    }
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

/// Arc as a polyline from fraction `a` to `b` of a full turn, starting at 12 o'clock.
fn arc(cx: f32, cy: f32, r: f32, a: f32, b: f32) -> Option<tiny_skia::Path> {
    let steps = ((b - a) * 120.0).ceil().max(2.0) as usize;
    let mut pb = PathBuilder::new();
    for i in 0..=steps {
        let t = a + (b - a) * i as f32 / steps as f32;
        let ang = t * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        let (px, py) = (cx + r * ang.cos(), cy + r * ang.sin());
        if i == 0 {
            pb.move_to(px, py)
        } else {
            pb.line_to(px, py)
        }
    }
    pb.finish()
}

// ------------------------------------------------------------------ faces

// Night Signal palette.
pub const BG: Rgb = [17, 23, 29]; // #11171D
pub const IVORY: Rgb = [244, 241, 232];
pub const MINT: Rgb = [105, 224, 193];
pub const CORAL: Rgb = [255, 130, 120];
pub const AMBER: Rgb = [255, 191, 105];
pub const LAVENDER: Rgb = [183, 154, 247];
pub const MUTED: Rgb = [135, 144, 158];
pub const LINE: Rgb = [48, 54, 63];

pub const DEFAULT_BG: Rgb = BG;

/// Semantic tone a widget state asks for (data-tone in the widget spec).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tone {
    Active,
    Info,
    Attention,
    Error,
}

/// A widget's resolved theme: per-widget override > palette > Night Signal default.
#[derive(Clone, Debug)]
pub struct Th {
    pub accent: Rgb,
    pub light: Rgb,
    pub light_opacity: f32,
    pub idle_factor: f32,
    pub bg: Rgb,
    pub border: Option<Rgb>,
    pub text: Rgb,
    pub secondary: Rgb,
    pub icon: Option<Rgb>,
    pub icon_accent: Option<Rgb>,
    pub chart: Rgb,
    pub progress: Rgb,
    pub track: Rgb,
    pub up: Rgb,
    pub down: Rgb,
    pub cover_tint: Option<Rgb>,
    pub airy: bool,
}

pub fn hex(c: Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

impl Th {
    pub fn resolve(theme: &crate::config::Theme, widget: &str, tone: Tone) -> Th {
        let pal = &theme.palette;
        let col = |s: &str, d: Rgb| parse_color(s).unwrap_or(d);
        let tone_col = |t: Tone| match t {
            Tone::Active => col(&pal.active, MINT),
            Tone::Info => col(&pal.info, LAVENDER),
            Tone::Attention => col(&pal.attention, AMBER),
            Tone::Error => col(&pal.error, CORAL),
        };
        let w = theme.widgets.get(widget).cloned().unwrap_or_default();
        let opt = |o: &Option<String>| o.as_deref().and_then(parse_color);
        // An error state keeps its warning colour unless the widget overrides it explicitly.
        let accent = opt(&w.accent).filter(|_| tone != Tone::Error).unwrap_or_else(|| tone_col(tone));
        Th {
            accent,
            light: opt(&w.light).unwrap_or(accent),
            light_opacity: w.light_opacity.unwrap_or(0.9).clamp(0.0, 1.0),
            idle_factor: w.idle_light_factor.unwrap_or(0.28).clamp(0.0, 1.0),
            bg: opt(&w.background).unwrap_or_else(|| col(&pal.background, BG)),
            border: opt(&w.border),
            text: opt(&w.text).unwrap_or(IVORY),
            secondary: opt(&w.secondary).unwrap_or([155, 165, 178]),
            icon: opt(&w.icon_color),
            icon_accent: opt(&w.icon_accent),
            chart: opt(&w.chart).unwrap_or(accent),
            progress: opt(&w.progress).unwrap_or(accent),
            track: opt(&w.track).unwrap_or(LINE),
            up: opt(&w.network_up).unwrap_or_else(|| tone_col(Tone::Active)),
            down: opt(&w.network_down).unwrap_or_else(|| tone_col(Tone::Info)),
            cover_tint: opt(&w.cover_tint),
            airy: w.layout.as_deref() == Some("airy"),
        }
    }

    /// Icon colour overrides (neutral strokes, accented strokes) for set icons.
    pub fn icon_overrides(&self) -> Vec<(String, String)> {
        let mut v = vec![];
        if let Some(c) = self.icon {
            v.push(("icon-base".to_string(), hex(c)));
        }
        match self.icon_accent {
            Some(c) => {
                for k in ["icon-mint", "icon-lavender", "icon-amber", "icon-coral"] {
                    v.push((k.to_string(), hex(c)));
                }
            }
            // --widget-icon-accent defaults to the widget accent (the icons' active role)
            None if self.accent != MINT => v.push(("icon-mint".to_string(), hex(self.accent))),
            None => {}
        }
        v
    }

    /// Background: surface, tinted light from the upper left, optional border.
    pub fn surface(&self, c: &mut Canvas, idle: bool) {
        let k = if idle { self.idle_factor } else { 1.0 };
        c.glow(self.light, self.light_opacity * k * 0.27);
        if let Some(b) = self.border {
            let (w, h) = (c.w(), c.h());
            c.stroke_rect(0.0, 0.0, w, h, 0.0, 2.0, b);
        }
    }
}
pub const DEFAULT_FG: Rgb = IVORY;

pub fn look_colors(look: &Look, default_bg: Rgb) -> (Rgb, Rgb) {
    (
        look.bg.as_deref().and_then(parse_color).unwrap_or(default_bg),
        look.fg.as_deref().and_then(parse_color).unwrap_or(DEFAULT_FG),
    )
}

/// A button. Layouts: full image; icon + label; label only (big); `value` (a remote
/// key's state) as a big figure when numeric, else as a pill.
pub fn button(f: &Fonts, base: &Path, look: &Look, value: Option<&str>, size: (u32, u32), default_bg: Rgb, env: Option<&IconEnv>) -> Canvas {
    let (bg, fg) = look_colors(look, default_bg);
    let mut c = Canvas::new(size.0, size.1, bg);
    if let Some(img) = look.image.as_deref().and_then(|p| image::open(resolve(base, p)).ok()) {
        c.image_cover(&img);
        return c;
    }
    let (w, h) = (c.w(), c.h());
    let cx = w / 2.0;
    let label = look.label.clone().unwrap_or_default().replace("\\n", "\n");
    let lines: Vec<&str> = label.lines().filter(|l| !l.trim().is_empty()).collect();
    let icon = look.icon.as_deref().filter(|i| !i.trim().is_empty());
    let value = value.filter(|v| !v.trim().is_empty());
    let numeric = value.is_some_and(|v| v.trim_start_matches('-').chars().next().is_some_and(|ch| ch.is_ascii_digit()));
    let pill_col = |v: &str| match v.to_ascii_lowercase().as_str() {
        "on" | "open" | "home" | "playing" | "unlocked" | "active" | "heat" | "cool" => MINT,
        "unavailable" | "unknown" | "error" | "problem" => CORAL,
        _ => MUTED,
    };
    let max = w - 20.0;
    match (icon, lines.len(), value) {
        // label only
        (None, _, None) if !lines.is_empty() => {
            if lines.len() == 1 {
                c.caps(f, lines[0], 26.0, cx, h / 2.0 + 9.0, fg, max);
            } else {
                let top = h / 2.0 - (lines.len() as f32 - 1.0) * 17.0 + 8.0;
                for (i, l) in lines.iter().enumerate() {
                    c.caps(f, l, 22.0, cx, top + i as f32 * 34.0, fg, max);
                }
            }
        }
        (None, _, Some(v)) => {
            if numeric {
                c.figure(f, v, 56.0, cx, 112.0, fg, max);
            } else {
                c.pill(f, v, cx, 96.0, pill_col(v));
            }
            if let Some(l) = lines.first() {
                c.caps(f, l, 18.0, cx, 162.0, fg, max);
            }
        }
        (Some(i), 0, None) => c.icon_s(f, base, i, env, cx - 60.0, h / 2.0 - 60.0, 120.0, 120.0, fg),
        (Some(i), n, None) => {
            if n == 1 {
                c.icon_s(f, base, i, env, cx - 52.0, 22.0, 104.0, 104.0, fg);
                c.caps(f, lines[0], 22.0, cx, 168.0, fg, max);
            } else {
                c.icon_s(f, base, i, env, cx - 44.0, 14.0, 88.0, 88.0, fg);
                c.caps(f, lines[0], 19.0, cx, 140.0, fg, max);
                c.caps(f, lines[1], 19.0, cx, 168.0, fg, max);
            }
        }
        (Some(i), _, Some(v)) => {
            c.icon_s(f, base, i, env, cx - 44.0, 12.0, 88.0, 88.0, fg);
            if numeric {
                c.figure(f, v, 36.0, cx, 140.0, fg, max);
            } else {
                c.pill(f, v, cx, 126.0, pill_col(v));
            }
            if let Some(l) = lines.first() {
                c.caps(f, l, 16.0, cx, 178.0, fg, max);
            }
        }
        _ => {}
    }
    c
}

thread_local! {
    /// Icon id -> the icon drawn instead, while the engine draws one built-in face (`IconSwap`).
    static ICON_SWAP: std::cell::RefCell<Vec<(String, String)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// While it lives, the built-in faces draw `to` wherever they would draw `from` (both icon ids).
pub struct IconSwap;

impl IconSwap {
    pub fn set(swaps: Vec<(String, String)>) -> IconSwap {
        ICON_SWAP.with(|s| *s.borrow_mut() = swaps);
        IconSwap
    }
}

impl Drop for IconSwap {
    fn drop(&mut self) {
        ICON_SWAP.with(|s| s.borrow_mut().clear());
    }
}

/// What bound icons read while one face is drawn, and what they did read.
pub struct IconEnv<'a> {
    pub ctx: crate::bind::Ctx<'a>,
    pub reads: std::cell::RefCell<crate::bind::Reads>,
}

fn empty_map() -> &'static serde_json::Map<String, serde_json::Value> {
    static M: std::sync::OnceLock<serde_json::Map<String, serde_json::Value>> = std::sync::OnceLock::new();
    M.get_or_init(serde_json::Map::new)
}

fn no_signals() -> &'static crate::bind::Signals {
    static S: std::sync::OnceLock<crate::bind::Signals> = std::sync::OnceLock::new();
    S.get_or_init(crate::bind::Signals::default)
}

impl IconEnv<'static> {
    /// Nothing to read: every binding takes its fallback.
    pub fn empty() -> IconEnv<'static> {
        IconEnv::with_props(empty_map())
    }
}

impl<'a> IconEnv<'a> {
    pub fn new(ctx: crate::bind::Ctx<'a>) -> IconEnv<'a> {
        IconEnv { ctx, reads: Default::default() }
    }
    /// Only static props (cards, remote keys).
    pub fn with_props(props: &'a serde_json::Map<String, serde_json::Value>) -> IconEnv<'a> {
        IconEnv::new(crate::bind::Ctx { implements: None, props: Some(props), config: empty_map(), signals: no_signals(), theme: empty_map() })
    }
}

/// A key whose app comes from a plugin that is not installed. Its settings stay.
pub fn missing_plugin(f: &Fonts, app: &str, size: (u32, u32)) -> Canvas {
    let mut c = Canvas::new(size.0, size.1, DEFAULT_BG);
    let (w, h) = (c.w(), c.h());
    // dashed frame
    let mut x = 10.0;
    while x < w - 10.0 {
        c.rect(x, 8.0, 12.0f32.min(w - 10.0 - x), 3.0, 0.0, AMBER, 1.0);
        c.rect(x, h - 11.0, 12.0f32.min(w - 10.0 - x), 3.0, 0.0, AMBER, 1.0);
        x += 22.0;
    }
    let mut y = 10.0;
    while y < h - 10.0 {
        c.rect(8.0, y, 3.0, 12.0f32.min(h - 10.0 - y), 0.0, AMBER, 1.0);
        c.rect(w - 11.0, y, 3.0, 12.0f32.min(h - 10.0 - y), 0.0, AMBER, 1.0);
        y += 22.0;
    }
    c.caps(f, "Plugin", 20.0, w / 2.0, h / 2.0 - 6.0, AMBER, w - 30.0);
    c.caps(f, "missing", 20.0, w / 2.0, h / 2.0 + 22.0, AMBER, w - 30.0);
    let name = app.split(':').next().unwrap_or(app);
    c.caps(f, name, 12.0, w / 2.0, h - 30.0, MUTED, w - 30.0);
    c
}

/// What the status key shows.
pub struct StatusInfo<'a> {
    pub profile: &'a str,
    pub page: &'a str,
    pub auto: bool,
    pub remote: bool,
    pub mqtt: Option<bool>,
    pub app: Option<&'a str>,
    /// Song for the right-hand column; the clock is shown otherwise.
    pub media: Option<(&'a crate::plugins::core::media::NowPlaying, Option<&'a DynamicImage>)>,
}

/// Status key: profile on the left, music or clock on the right.
pub fn status_face(f: &Fonts, s: &StatusInfo) -> Canvas {
    let mut c = Canvas::new(456, 196, BG);
    let accent = if s.remote { [3, 169, 244] } else { MINT };
    c.glow(accent, 0.16); // soft light from the upper left, no side stripe
    let (title, sub) = if s.remote {
        ("Remote".to_string(), "Home Assistant".to_string())
    } else {
        let sub = match s.app {
            Some(a) if s.auto => format!("Auto · {a}"),
            _ if s.auto => "Auto".to_string(),
            _ => "Manual profile".to_string(),
        };
        (s.profile.to_string(), sub)
    };
    // left column
    let lx = 30.0;
    let lw = 268.0;
    let t = title.to_uppercase();
    let mut size = 62.0;
    while size > 26.0 && c.tw(f, &t, size, Weight::Bold, 0.5) > lw {
        size -= 2.0;
    }
    c.tl(f, &t, size, lx, 84.0, IVORY, Weight::Bold, 0.5);
    let sub = c.fit(f, &sub.to_uppercase(), 16.0, lw - 10.0);
    c.tl(f, &sub, 16.0, lx, 118.0, MUTED, Weight::Semi, 2.2);
    c.hline(lx, lx + lw, 140.0, LINE);
    match s.mqtt {
        Some(ok) => {
            c.rect(lx, 160.0, 16.0, 16.0, 8.0, if ok { MINT } else { CORAL }, 1.0);
            c.caps_at(f, "Home Assistant", 15.0, lx + 28.0, 174.0, IVORY);
        }
        None => {
            c.caps_at(f, &format!("Page · {}", s.page), 15.0, lx, 174.0, MUTED);
        }
    }
    // right column
    c.vline(318.0, 24.0, 172.0, LINE);
    let rx = 387.0;
    match s.media {
        Some((np, art)) => {
            let (ax, ay, a) = (rx - 46.0, 20.0, 92.0);
            match art {
                Some(img) => c.image_cover_at(img, ax, ay, a, a),
                None => {
                    c.rect(ax, ay, a, a, 8.0, [30, 35, 42], 1.0);
                    c.set_icon("now-playing", ax + 18.0, ay + 18.0, 56.0, 56.0, &[]);
                }
            }
            c.stroke_rect(ax, ay, a, a, 8.0, 1.5, LINE);
            let title = c.fit(f, &np.title.to_uppercase(), 15.0, 120.0);
            let tw = c.tw(f, &title, 15.0, Weight::Bold, 0.6);
            c.tl(f, &title, 15.0, rx - tw / 2.0, 140.0, IVORY, Weight::Bold, 0.6);
            let artist = c.fit(f, &np.artist.to_uppercase(), 12.0, 120.0);
            c.caps(f, &artist, 12.0, rx, 160.0, MUTED, 120.0);
            if np.duration > 0.0 {
                let frac = (np.position / np.duration).clamp(0.0, 1.0) as f32;
                c.rect(rx - 56.0, 172.0, 112.0, 3.0, 1.5, LINE, 1.0);
                c.rect(rx - 56.0, 172.0, (112.0 * frac).max(3.0), 3.0, 1.5, if np.playing { MINT } else { MUTED }, 1.0);
            }
        }
        None => {
            let now = chrono::Local::now();
            c.figure(f, &now.format("%H:%M").to_string(), 40.0, rx, 98.0, IVORY, 120.0);
            c.caps(f, &now.format("%a %-d %b").to_string(), 13.0, rx, 128.0, MUTED, 120.0);
        }
    }
    c
}

/// A notification shown on the status key.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Notification {
    #[serde(default, alias = "message")]
    pub text: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    /// "slide" (default), "flash", "pulse" or "scroll". Long text always scrolls.
    #[serde(default)]
    pub style: Option<String>,
    /// Seconds on screen (default 8).
    #[serde(default)]
    pub duration: Option<f32>,
    /// "critical" (stays until dismissed, wakes the screensaver), "high", "normal", "low".
    #[serde(default)]
    pub priority: Option<String>,
}

impl Notification {
    pub fn simple(text: &str, icon: Option<&str>, color: Option<&str>) -> Self {
        Notification {
            text: text.into(),
            title: None,
            icon: icon.map(Into::into),
            color: color.map(Into::into),
            style: None,
            duration: None,
            priority: None,
        }
    }
    pub fn critical(&self) -> bool {
        self.priority.as_deref() == Some("critical")
    }
    pub fn duration(&self) -> f32 {
        if self.critical() {
            return f32::INFINITY;
        }
        self.duration.unwrap_or(8.0).clamp(1.0, 120.0)
    }
}

fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// One animation frame, `t` seconds into the notification.
pub fn notification_frame(f: &Fonts, base: &Path, n: &Notification, t: f32) -> Canvas {
    let color = n.color.as_deref().and_then(parse_color).unwrap_or(MINT);
    let dur = n.duration();
    let style = n.style.as_deref().unwrap_or("slide");
    let glow = match style {
        "flash" if t < 1.6 => {
            if ((t / 0.2) as i32) % 2 == 0 {
                0.34
            } else {
                0.06
            }
        }
        "pulse" => 0.08 + 0.14 * (0.5 + 0.5 * (t * std::f32::consts::TAU / 1.2).sin()),
        _ => 0.10,
    };
    let bg = mix(BG, color, glow);
    let mut c = Canvas::new(456, 196, bg);
    let (w, h) = (c.w(), c.h());
    c.rect(0.0, 0.0, 6.0, h, 0.0, color, 1.0);
    c.rect(6.0, h - 4.0, (w - 6.0) * (1.0 - t / dur).max(0.0), 4.0, 0.0, color, 1.0);

    let enter = if style == "slide" || style == "scroll" { (1.0 - ease_out(t / 0.45)) * w } else { 0.0 };
    let leave = if t > dur - 0.4 { ease_out((t - (dur - 0.4)) / 0.4) * -w } else { 0.0 };
    let dx = enter + leave;

    let has_icon = n.icon.as_deref().is_some_and(|i| !i.trim().is_empty());
    let text_x = if has_icon { 154.0 } else { 34.0 };
    if let Some(icon) = n.icon.as_deref().filter(|_| has_icon) {
        let bounce = if t < 0.8 { (t * 14.0).sin() * 8.0 * (1.0 - t / 0.8) } else { 0.0 };
        c.icon(f, base, icon, 30.0 + dx, 46.0 - bounce, 104.0, 104.0, IVORY);
    }
    let avail = w - text_x - 22.0;
    let title = n.title.as_deref().filter(|t| !t.is_empty());
    let body = n.text.as_str();
    let body_size = if title.is_some() { 36.0 } else { 42.0 };
    let body_y = if title.is_some() { 132.0 } else { 116.0 };
    if let Some(tt) = title {
        let tt = c.fit(f, &tt.to_uppercase(), 16.0, avail);
        c.tl(f, &tt, 16.0, text_x + dx, 78.0, color, Weight::Semi, 2.4);
    }
    let bw = c.tw(f, body, body_size, Weight::Bold, 0.0);
    if bw <= avail {
        c.tl(f, body, body_size, text_x + dx, body_y, IVORY, Weight::Bold, 0.0);
    } else {
        // Marquee through the free area after a short pause.
        let travel = bw - avail + 40.0;
        let off = ((t - 0.8).max(0.0) * 120.0).min(travel);
        let mut strip = Canvas::new(avail as u32, 60, bg);
        strip.tl(f, body, body_size, -off, 44.0, IVORY, Weight::Bold, 0.0);
        c.pm.draw_pixmap((text_x + dx) as i32, (body_y - 44.0) as i32, strip.pm.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    }
    c
}

/// PNG bytes for the app's preview grid.
pub fn png(rgb: Vec<u8>, size: (u32, u32)) -> Vec<u8> {
    let img = image::RgbImage::from_raw(size.0, size.1, rgb).expect("size");
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).expect("png encode");
    out.into_inner()
}

/// PNG from premultiplied RGBA (a tiny-skia pixmap), keeping transparency.
pub fn png_rgba(mut rgba: Vec<u8>, size: (u32, u32)) -> Vec<u8> {
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    let img = image::RgbaImage::from_raw(size.0, size.1, rgba).expect("size");
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).expect("png encode");
    out.into_inner()
}
