//! Night Signal seasonal idle scene (port of the "A little weather" prototype).
//! Shared by the desktop app and d200fw, which draws it on the device itself.
//!
//! One scene is drawn across the whole 1280 × 720 panel in shared coordinates,
//! so particles travel naturally from key to key; each key shows its window of
//! it. Steady, low-contrast motion: snow, spring petals, a beach tide, autumn
//! leaves or rain, plus one small visitor on the wide key.

pub use tiny_skia;
use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, Point, Rect, SpreadMode, Stroke, Transform,
};

pub const W: f32 = 1280.0;
pub const H: f32 = 720.0;
/// The prototype's units are CSS pixels on a ~600 px wide device.
const K: f32 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    Snow,
    Pink,
    Beach,
    Leaves,
    Rain,
}

impl Season {
    /// A scene by name; `None` for "auto" or anything unknown.
    pub fn from_name(s: &str) -> Option<Season> {
        match s {
            "snow" => Some(Season::Snow),
            "pink" | "spring" => Some(Season::Pink),
            "beach" | "summer" => Some(Season::Beach),
            "leaves" | "autumn" => Some(Season::Leaves),
            "rain" => Some(Season::Rain),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Season::Snow => "snow",
            Season::Pink => "pink",
            Season::Beach => "beach",
            Season::Leaves => "leaves",
            Season::Rain => "rain",
        }
    }
}


struct P {
    x: f32,
    y: f32,
    size: f32,
    speed: f32,
    phase: f32,
    spin: f32,
}

pub struct Scene {
    particles: Vec<P>,
}

fn rgba(hex: u32, a: f32) -> Color {
    Color::from_rgba8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8, (a.clamp(0.0, 1.0) * 255.0) as u8)
}

fn paint(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

impl Scene {
    pub fn new() -> Scene {
        // Same seeded generator as the prototype, so the scenes match.
        let mut seed: u32 = 723;
        let mut rand = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed as f32 / 4294967296.0
        };
        let particles = (0..110)
            .map(|_| P {
                x: rand(),
                y: rand(),
                size: 0.7 + rand() * 1.6,
                speed: 0.012 + rand() * 0.028,
                phase: rand() * std::f32::consts::TAU,
                spin: (rand() - 0.5) * 1.1,
            })
            .collect();
        Scene { particles }
    }

    /// Draw the scene at time `t` seconds into a 1280 × 720 pixmap.
    pub fn render(&self, season: Season, t: f32, pm: &mut Pixmap) {
        let bg = match season {
            Season::Beach => 0x2b2a29,
            Season::Pink => 0x18151e,
            Season::Leaves => 0x19191c,
            _ => 0x101a23,
        };
        pm.fill(rgba(bg, 1.0));
        let id = Transform::identity();
        match season {
            Season::Beach => {
                let sand = LinearGradient::new(
                    Point::from_xy(0.0, 0.0),
                    Point::from_xy(0.0, H),
                    vec![GradientStop::new(0.0, rgba(0x292827, 1.0)), GradientStop::new(1.0, rgba(0x82725f, 1.0))],
                    SpreadMode::Pad,
                    id,
                );
                if let Some(shader) = sand {
                    let mut p = Paint::default();
                    p.shader = shader;
                    pm.fill_rect(Rect::from_xywh(0.0, 0.0, W, H).unwrap(), &p, id, None);
                }
                for q in self.particles.iter().take(20) {
                    dot(pm, q.x * W, q.y * H, 0.7 * K, rgba(0xb5a48c, 0.33));
                }
                let tide = H * (0.63 + (t * 0.19).sin() * 0.08);
                wave(pm, tide, 9.0 * K, t * 0.33, rgba(0x6b9d9f, 1.0), Some(rgba(0xd9e0cf, 0.47)));
                wave(pm, tide + 23.0 * K, 10.0 * K, t * 0.28 + 0.5, rgba(0x427d88, 1.0), Some(rgba(0xc6ded4, 0.33)));
                wave(pm, tide + 58.0 * K, 13.0 * K, t * 0.23 + 1.0, rgba(0x285c70, 1.0), None);
            }
            Season::Rain => {
                let stroke = Stroke { width: 0.7 * K, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
                let drop = paint(rgba(0x9dbcca, 0.33));
                for q in &self.particles {
                    let x = (q.x * W - t * 15.0 * K + W * 20.0).rem_euclid(W + 40.0 * K);
                    let y = ((q.y + t * q.speed * 3.0).fract()) * (H + 45.0 * K) - 20.0 * K;
                    let mut pb = PathBuilder::new();
                    pb.move_to(x, y);
                    pb.line_to(x - 4.0 * K, y + 13.0 * q.size * K);
                    if let Some(path) = pb.finish() {
                        pm.stroke_path(&path, &drop, &stroke, id, None);
                    }
                }
                for q in self.particles.iter().take(9) {
                    let phase = (t * 0.4 + q.phase).rem_euclid(1.0);
                    let (rx, ry) = ((4.0 + phase * 17.0) * K, (1.0 + phase * 3.0) * K);
                    let (cx, cy) = (q.x * W, H * (0.5 + q.y * 0.47));
                    if let Some(r) = Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0) {
                        if let Some(path) = PathBuilder::from_oval(r) {
                            let s = Stroke { width: 1.0 * K, ..Default::default() };
                            pm.stroke_path(&path, &paint(rgba(0x9dbcca, (1.0 - phase) * 0.13)), &s, id, None);
                        }
                    }
                }
            }
            _ => {
                let count = if season == Season::Snow { 90 } else { 40 };
                for (n, q) in self.particles.iter().take(count).enumerate() {
                    let speed = if season == Season::Snow { 2.0 } else { 4.0 };
                    let drift = ((t * 0.22 + q.phase).sin() * 12.0 + t * speed) * K;
                    let span = W + 35.0 * K;
                    let x = (q.x * W + drift).rem_euclid(span) - 15.0 * K;
                    let y = ((q.y + t * q.speed * 0.45).fract()) * (H + 35.0 * K) - 15.0 * K;
                    if season == Season::Snow {
                        dot(pm, x, y, q.size * 0.85 * K, rgba(0xdae7e1, 0.73));
                    } else {
                        let pink = season == Season::Pink;
                        let colors: [u32; 3] = if pink { [0xdbaac1, 0xe9bed0, 0xb68fae] } else { [0xcf9966, 0xb97854, 0xe0ad77] };
                        let size = q.size * if pink { 3.4 } else { 4.0 } * K;
                        let angle = (t * 0.6 + q.phase).sin() + t * q.spin;
                        leaf(pm, x, y, size, angle, rgba(colors[n % 3], 1.0), pink);
                    }
                }
            }
        }
        visitor(pm, season, t);
    }
}

fn dot(pm: &mut Pixmap, x: f32, y: f32, r: f32, c: Color) {
    if let Some(p) = PathBuilder::from_circle(x, y, r.max(0.3)) {
        pm.fill_path(&p, &paint(c), FillRule::Winding, Transform::identity(), None);
    }
}

fn leaf(pm: &mut Pixmap, x: f32, y: f32, size: f32, angle: f32, c: Color, petal: bool) {
    let tr = Transform::from_translate(x, y).pre_concat(Transform::from_rotate(angle.to_degrees()));
    let mut pb = PathBuilder::new();
    pb.move_to(-size, 0.0);
    pb.cubic_to(-size * 0.3, -size * 0.8, size * 0.8, -size * 0.6, size, 0.0);
    pb.cubic_to(size * 0.2, size * 0.8, -size * 0.4, size * 0.7, -size, 0.0);
    pb.close();
    if let Some(p) = pb.finish() {
        pm.fill_path(&p, &paint(c), FillRule::Winding, tr, None);
    }
    if !petal {
        let mut pb = PathBuilder::new();
        pb.move_to(-size * 0.7, 0.0);
        pb.line_to(size * 0.7, 0.0);
        if let Some(p) = pb.finish() {
            let s = Stroke { width: 0.6 * K, ..Default::default() };
            pm.stroke_path(&p, &paint(rgba(0x4b302c, 0.53)), &s, tr, None);
        }
    }
}

fn wave(pm: &mut Pixmap, y: f32, amp: f32, phase: f32, fill: Color, foam: Option<Color>) {
    let yy = |x: f32| y + (x / W * 8.0 + phase).sin() * amp + (x / W * 17.0 - phase * 0.6).sin() * amp * 0.35;
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, H);
    pb.line_to(0.0, yy(0.0));
    let mut x = 0.0;
    while x <= W + 16.0 {
        pb.line_to(x, yy(x));
        x += 16.0;
    }
    pb.line_to(W, H);
    pb.close();
    if let Some(p) = pb.finish() {
        pm.fill_path(&p, &paint(fill), FillRule::Winding, Transform::identity(), None);
    }
    if let Some(foam) = foam {
        let mut pb = PathBuilder::new();
        pb.move_to(0.0, yy(0.0));
        let mut x = 16.0;
        while x <= W + 16.0 {
            pb.line_to(x, yy(x));
            x += 16.0;
        }
        if let Some(p) = pb.finish() {
            let s = Stroke { width: 1.4 * K, ..Default::default() };
            pm.stroke_path(&p, &paint(foam), &s, Transform::identity(), None);
        }
    }
}

/// The miniature seasonal character, wandering on the wide key.
fn visitor(pm: &mut Pixmap, season: Season, t: f32) {
    // wide key window: x 791..1247, y 521..717
    let (rx, ry, rw, rh): (f32, f32, f32, f32) = (791.0, 521.0, 456.0, 196.0);
    let s = (rh / 100.0).min(2.2);
    let x = rx + rw * (0.5 + (t * 0.13).sin() * 0.24);
    let y = ry + rh * 0.66 + (t * 0.7).sin() * 2.0 * K;
    let tr = Transform::from_translate(x, y).pre_scale(s, s);
    let fill = |pm: &mut Pixmap, path: Option<tiny_skia::Path>, c: u32| {
        if let Some(p) = path {
            pm.fill_path(&p, &paint(rgba(c, 1.0)), FillRule::Winding, tr, None);
        }
    };
    let circle = |cx: f32, cy: f32, r: f32| PathBuilder::from_circle(cx, cy, r);
    let oval = |cx: f32, cy: f32, rx: f32, ry: f32| Rect::from_xywh(cx - rx, cy - ry, rx * 2.0, ry * 2.0).and_then(PathBuilder::from_oval);
    let line = |pm: &mut Pixmap, pts: &[(f32, f32, f32, f32)], c: u32| {
        let mut pb = PathBuilder::new();
        for (x1, y1, x2, y2) in pts {
            pb.move_to(*x1, *y1);
            pb.line_to(*x2, *y2);
        }
        if let Some(p) = pb.finish() {
            let st = Stroke { width: 2.0, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
            pm.stroke_path(&p, &paint(rgba(c, 1.0)), &st, tr, None);
        }
    };
    match season {
        Season::Snow => {
            fill(pm, circle(0.0, 5.0, 13.0), 0xd8e5e8);
            fill(pm, circle(0.0, -12.0, 9.0), 0xe6eef0);
            fill(pm, circle(-3.0, -14.0, 1.0), 0x1d2a31);
            fill(pm, circle(3.0, -14.0, 1.0), 0x1d2a31);
            let mut nose = PathBuilder::new();
            nose.move_to(0.0, -11.0);
            nose.line_to(10.0, -9.0);
            nose.line_to(0.0, -7.0);
            nose.close();
            fill(pm, nose.finish(), 0xe4a773);
            fill(pm, Rect::from_xywh(-11.0, -3.0, 22.0, 4.0).map(PathBuilder::from_rect), 0xb79af7);
            line(pm, &[(-11.0, 2.0, -22.0, -7.0), (11.0, 2.0, 22.0, -7.0)], 0xc1a58c);
        }
        Season::Beach => {
            let mut legs = vec![];
            for side in [-1.0f32, 1.0] {
                for k in 0..3 {
                    let k = k as f32;
                    legs.push((side * 7.0, k * 4.0, side * (16.0 + (t * 2.0 + k).sin() * 2.0), k * 5.0 - 3.0));
                }
                legs.push((side * 7.0, -2.0, side * 15.0, -11.0));
            }
            line(pm, &legs, 0xe9aa8b);
            fill(pm, circle(-15.0, -13.0, 4.0), 0xe0a087);
            fill(pm, circle(15.0, -13.0, 4.0), 0xe0a087);
            fill(pm, oval(0.0, 2.0, 12.0, 8.0), 0xd79880);
            fill(pm, circle(-5.0, -6.0, 2.0), 0xe9e5d7);
            fill(pm, circle(5.0, -6.0, 2.0), 0xe9e5d7);
            fill(pm, circle(-5.0, -6.0, 1.0), 0x24333c);
            fill(pm, circle(5.0, -6.0, 1.0), 0x24333c);
        }
        Season::Pink => {
            // butterfly: wings flap by squashing horizontally
            let f = 0.4 + (t * 2.0).sin().abs() * 0.6;
            let wings = Transform::from_translate(x, y).pre_scale(s * f, s);
            for (cx, cy, r, c) in [(-8.0, -5.0, 9.0, 0xdda8bb), (8.0, -5.0, 9.0, 0xe6b7ca), (-6.0, 6.0, 6.0, 0xc997b2), (6.0, 6.0, 6.0, 0xc997b2)] {
                if let Some(p) = circle(cx, cy, r) {
                    pm.fill_path(&p, &paint(rgba(c, 1.0)), FillRule::Winding, wings, None);
                }
            }
            line(pm, &[(0.0, -10.0, 0.0, 10.0)], 0xeddbc8);
        }
        Season::Leaves => {
            fill(pm, oval(0.0, 2.0, 10.0, 14.0), 0xc79569);
            fill(pm, oval(0.0, -8.0, 13.0, 5.0), 0x826650);
            line(pm, &[(0.0, -11.0, 2.0, -17.0)], 0xb69c7e);
            fill(pm, circle(-3.0, 1.0, 1.0), 0x332a23);
            fill(pm, circle(3.0, 1.0, 1.0), 0x332a23);
        }
        Season::Rain => {
            fill(pm, oval(0.0, 9.0, 12.0, 8.0), 0xe0cc87);
            fill(pm, circle(7.0, 0.0, 7.0), 0xe0cc87);
            fill(pm, circle(9.0, -2.0, 1.0), 0x313231);
            fill(pm, Rect::from_xywh(12.0, 0.0, 6.0, 3.0).map(PathBuilder::from_rect), 0xd99a70);
            line(pm, &[(-9.0, 7.0, -9.0, -27.0)], 0xb8c4c8);
            // umbrella: half disc
            let mut pb = PathBuilder::new();
            let (cx, cy, r) = (-9.0, -25.0, 19.0);
            pb.move_to(cx - r, cy);
            for i in 0..=24 {
                let a = std::f32::consts::PI + std::f32::consts::PI * i as f32 / 24.0;
                pb.line_to(cx + r * a.cos(), cy + r * a.sin());
            }
            pb.close();
            fill(pm, pb.finish(), 0xa89dc6);
        }
    }
}

/// Landscape window (x, y, w, h) of key `k` on the panel.
pub fn key_rect(k: usize) -> (usize, usize, usize, usize) {
    match k {
        0..=9 => (11 + 260 * (k % 5), 3 + 259 * (k / 5), 196, 196),
        10..=12 => (11 + 260 * (k - 10), 521, 196, 196),
        _ => (791, 521, 456, 196),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenes_render_and_move() {
        let sc = Scene::new();
        for s in [Season::Snow, Season::Pink, Season::Beach, Season::Leaves, Season::Rain] {
            let mut a = Pixmap::new(W as u32, H as u32).unwrap();
            let mut b = Pixmap::new(W as u32, H as u32).unwrap();
            sc.render(s, 10.0, &mut a);
            sc.render(s, 10.5, &mut b);
            assert_ne!(a.data(), b.data(), "{s:?} does not move");
            assert_eq!(Season::from_name(s.name()), Some(s));
        }
        assert_eq!(Season::from_name("auto"), None);
    }
}
