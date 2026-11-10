//! Wide-display cards (the status key), per the Night Signal widget spec.
//!
//! The wide key shows ONE thing at a time: its default content (the profile, or
//! an app such as Spotify) or a notification card. Cards have a priority:
//! critical > meeting > printer > push > email. Ordinary cards stay 8 s, then the
//! underlying content returns; critical cards stay until dismissed.
//!
//! Visuals: covers fill the left 42 % edge to edge; app symbols take 27 %;
//! text is inset 3 %; state emphasis is a soft light from the upper left, tinted
//! to the accent and faded once on arrival (no side stripe, no flashing).

use crate::render::{hex, mix, parse_color, Canvas, Fonts, Rgb, Th, Tone, Weight, AMBER, BG, CORAL, LAVENDER, MINT, MUTED};
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const W: f32 = 456.0;
pub const H: f32 = 196.0;
const INSET: f32 = 14.0; // 3 %
const COVER: f32 = 192.0; // 42 %
const SYMBOL: f32 = 123.0; // 27 %

/// What a card is about. Decides priority, accent and layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[default]
    Push,
    Email,
    Meeting,
    Printer,
    Steam,
    Media,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Card {
    #[serde(default)]
    pub kind: Kind,
    /// Small caps line ("NEW MAIL", "HOME ASSISTANT").
    #[serde(default)]
    pub source: Option<String>,
    /// Dominant line (sender, "In 5 min", "62%", game).
    #[serde(default)]
    pub title: String,
    /// Secondary line.
    #[serde(default)]
    pub detail: Option<String>,
    /// Small note at the bottom right ("2 unread", "18m remaining").
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    /// Interface properties for the icon, e.g. {"progress.value": 40}.
    #[serde(default)]
    pub props: serde_json::Map<String, serde_json::Value>,
    /// Older form of `props` (the `setIconState` object), still accepted from MQTT.
    #[serde(default, skip_serializing)]
    pub icon_state: Option<serde_json::Value>,
    #[serde(default)]
    pub color: Option<String>,
    /// 0..1 progress (printer, media).
    #[serde(default)]
    pub progress: Option<f32>,
    /// Opened on a short press.
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub critical: bool,
    /// Seconds on screen (default 8; critical: until dismissed).
    #[serde(default)]
    pub duration: Option<f32>,
    /// Groups cards that replace each other ("email", "print:<job>", "meeting:<id>").
    #[serde(default)]
    pub group: Option<String>,
}

impl Card {
    pub fn priority(&self) -> u8 {
        if self.critical {
            return 9;
        }
        match self.kind {
            Kind::Meeting => 5,
            Kind::Printer => 4,
            Kind::Push | Kind::Steam | Kind::Media => 3,
            Kind::Email => 2,
        }
    }
    pub fn duration(&self) -> f32 {
        if self.critical {
            f32::INFINITY
        } else {
            self.duration.unwrap_or(8.0).clamp(1.0, 600.0)
        }
    }
    pub fn accent(&self) -> Rgb {
        if let Some(c) = self.color.as_deref().and_then(parse_color) {
            return c;
        }
        if self.critical {
            return CORAL;
        }
        match self.kind {
            Kind::Meeting => AMBER,
            Kind::Steam => LAVENDER,
            _ => MINT,
        }
    }
}

impl From<&crate::render::Notification> for Card {
    fn from(n: &crate::render::Notification) -> Card {
        Card {
            kind: Kind::Push,
            source: Some(n.title.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "Notification".into())),
            title: n.text.clone(),
            icon: n.icon.clone(),
            color: n.color.clone(),
            critical: n.critical(),
            duration: n.duration,
            ..Default::default()
        }
    }
}

/// Printer state as published on `<base>/printer` (e.g. by a Home Assistant automation).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Printer {
    /// heating | printing | paused | complete | error | offline | idle
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub percent: Option<f32>,
    #[serde(default)]
    pub remaining_min: Option<f32>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub job: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    /// AMS spools: [{"color": "#69E0C1", "progress": 80}, ...] (remaining filament %).
    #[serde(default)]
    pub spools: Option<Vec<serde_json::Value>>,
    /// "ams" (default) or "ams-lite" for the spool icon.
    #[serde(default)]
    pub ams: Option<String>,
}

impl Printer {
    /// With spool data, the card shows the AMS with its four filaments.
    fn ams_icon(&self) -> Card {
        match &self.spools {
            Some(sp) if !sp.is_empty() && !matches!(self.state.to_ascii_lowercase().as_str(), "error" | "complete") => Card {
                icon: Some(if self.ams.as_deref() == Some("ams-lite") { "bambu-ams-lite".into() } else { "bambu-ams".into() }),
                props: crate::migrate::legacy_props("bambu-ams", &serde_json::json!({ "spools": sp })),
                ..Default::default()
            },
            _ => Card::default(),
        }
    }

    pub fn label(&self) -> String {
        let mut s = self.state.trim().to_string();
        if let Some(c) = s.get_mut(0..1) {
            c.make_ascii_uppercase();
        }
        if s.is_empty() { "Unknown".into() } else { s }
    }

    pub fn card(&self) -> Card {
        let st = self.state.to_ascii_lowercase();
        let remaining = self.remaining_min.filter(|_| st == "printing" || st == "paused").map(|m| {
            if m >= 60.0 { format!("{}h {:02}m remaining", (m / 60.0) as u32, (m % 60.0) as u32) } else { format!("{m:.0}m remaining") }
        });
        // Only a real job percentage is shown; heating has no made-up progress.
        let pct = self.percent.filter(|_| st == "printing" || st == "paused" || st == "complete");
        Card {
            kind: Kind::Printer,
            source: self.name.clone(),
            title: match (st.as_str(), pct) {
                ("complete", _) => "Complete".into(),
                ("error", _) => "Error".into(),
                (_, Some(p)) => format!("{p:.0}%"),
                _ => self.label(),
            },
            detail: match st.as_str() {
                "complete" | "error" => self.job.clone().or(self.message.clone()),
                _ => Some(self.label()),
            },
            note: remaining,
            progress: pct.map(|p| p / 100.0),
            icon: self.ams_icon().icon.or(Some(match st.as_str() {
                "complete" => "print-complete",
                "error" => "print-error",
                "paused" => "print-paused",
                "offline" => "print-offline",
                _ => "bambu-x1",
            }.into())),
            props: self.ams_icon().props,
            color: (st == "error").then(|| "#FF8278".into()),
            group: Some(format!("print:{}", self.job.clone().unwrap_or_default())),
            ..Default::default()
        }
    }
}

/// Live data a card can use (cover art) and its resolved theme.
pub struct Extra<'a> {
    pub cover: Option<&'a DynamicImage>,
    pub th: Th,
}

/// Widget id and tone of a card (THEMING.md data-widget / data-tone).
pub fn theme_for(theme: &crate::config::Theme, card: &Card) -> Th {
    let widget = match card.kind {
        Kind::Email => "mail",
        Kind::Media => "music",
        Kind::Printer => "printer",
        Kind::Meeting => "meeting",
        Kind::Steam => "game",
        Kind::Push => "push",
    };
    let tone = if card.critical || card.color.as_deref().is_some_and(|c| c.eq_ignore_ascii_case("#FF8278")) {
        Tone::Error
    } else {
        match card.kind {
            Kind::Meeting => Tone::Attention,
            Kind::Steam => Tone::Info,
            Kind::Media if card.source.as_deref() == Some("Paused") => Tone::Attention,
            _ => Tone::Active,
        }
    };
    let mut th = Th::resolve(theme, widget, tone);
    // an explicit card colour (from HA or an action) wins over the theme
    if let Some(c) = card.color.as_deref().and_then(parse_color).filter(|_| tone != Tone::Error) {
        th.accent = c;
        th.light = c;
        th.progress = c;
    }
    th
}

/// Idle-looking states (nothing to act on) get the dimmer idle light.
fn idle(card: &Card) -> bool {
    matches!(card.title.as_str(), "Inbox clear" | "Not playing" | "Nothing playing" | "No meetings" | "—")
        || card.color.as_deref() == Some("#87909E")
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Two lines max, ellipsis on the second.
fn two_lines(c: &Canvas, f: &Fonts, text: &str, size: f32, max: f32) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut lines: Vec<String> = vec![];
    let mut cur = String::new();
    for w in words {
        let cand = if cur.is_empty() { w.to_string() } else { format!("{cur} {w}") };
        if c.tw(f, &cand, size, Weight::Bold, 0.0) <= max || cur.is_empty() {
            cur = cand;
        } else {
            lines.push(cur);
            cur = w.to_string();
            if lines.len() == 1 && c.tw(f, &format!("{cur}"), size, Weight::Bold, 0.0) > max {
                break;
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.len() > 2 {
        let rest = lines[1..].join(" ");
        lines.truncate(1);
        lines.push(rest);
    }
    if let Some(last) = lines.last_mut() {
        if c.tw(f, last, size, Weight::Bold, 0.0) > max {
            let mut t: Vec<char> = last.chars().collect();
            while !t.is_empty() && c.tw(f, &(t.iter().collect::<String>() + "…"), size, Weight::Bold, 0.0) > max {
                t.pop();
            }
            *last = t.iter().collect::<String>().trim_end().to_string() + "…";
        }
    }
    lines
}

/// Draw a card `t` seconds after it arrived (`t` large = settled).
pub fn render(f: &Fonts, base: &Path, card: &Card, extra: &Extra, t: f32) -> Canvas {
    let th = &extra.th;
    let accent = th.accent;
    let (ivory, secondary, line) = (th.text, th.secondary, th.track);
    let mut c = Canvas::new(W as u32, H as u32, th.bg);
    // Soft light, faded gently once on arrival.
    let k = if idle(card) { th.idle_factor } else { 1.0 };
    c.glow(th.light, th.light_opacity * k * (0.25 + 0.13 * (1.0 - ease(t / 1.2))));
    if let Some(b) = th.border {
        c.stroke_rect(0.0, 0.0, W, H, 0.0, 2.0, b);
    }
    let a = ease(t / 0.3); // 300 ms arrival
    let dy = (1.0 - a) * 5.0;
    // Compact: cover fills the left 42 % edge to edge. Airy: a 24 % square cover inside 4 % padding.
    let airy = th.airy;
    let pad = if airy { 18.0 } else { INSET };
    let (cover_box, symbol_box) = if airy {
        let s = 110.0;
        ((pad, (H - s) / 2.0, s, s), (pad, (H - s) / 2.0, s, s))
    } else {
        ((0.0, 0.0, COVER, H), (INSET + 8.0, (H - SYMBOL) / 2.0, SYMBOL - 16.0, SYMBOL - 16.0))
    };

    let has_cover = matches!(card.kind, Kind::Media | Kind::Steam) && extra.cover.is_some();
    let tx = if has_cover {
        cover_box.0 + cover_box.2 + pad
    } else {
        let icon = card.icon.clone().unwrap_or_else(|| default_icon(card).to_string());
        let mut o = th.icon_overrides();
        if card.kind == Kind::Meeting && th.icon.is_none() {
            o.push(("icon-base".into(), hex(accent)));
        }
        let ovr: Vec<(&str, &str)> = o.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let (x, y, w, h) = symbol_box;
        let props = match &card.icon_state {
            Some(st) if card.props.is_empty() => crate::migrate::legacy_props(&icon, st),
            _ => card.props.clone(),
        };
        let env = crate::render::IconEnv::with_props(&props);
        if !c.set_icon_env(&icon, x, y + dy, w, h, &ovr, Some(&env)) {
            c.icon(f, base, &icon, x, y + dy, w, h, ivory);
        }
        if airy { x + w + pad } else { INSET + SYMBOL + INSET }
    };
    if let (true, Some(img)) = (has_cover, extra.cover) {
        let (x, y, w, h) = cover_box;
        c.image_cover_at(img, x, y, w, h);
        if let Some(tint) = th.cover_tint {
            c.rect(x, y, w, h, 0.0, tint, 0.35);
        }
    }
    let max = W - tx - pad;

    // source line: dot or app icon + caps in the accent colour
    let mut y = 54.0 + dy;
    if let Some(src) = card.source.as_deref().filter(|s| !s.is_empty()) {
        let mut x = tx;
        if matches!(card.kind, Kind::Email | Kind::Steam) || card.critical {
            c.rect(x, y - 13.0, 12.0, 12.0, 6.0, accent, 1.0);
            x += 22.0;
        }
        let src = c.fit(f, &src.to_uppercase(), 15.0, max - (x - tx));
        c.tl(f, &src, 15.0, x, y, accent, Weight::Semi, 2.2);
        y += 12.0;
    }
    // title: big, up to two lines
    let big = matches!(card.kind, Kind::Printer | Kind::Meeting) && card.title.chars().count() <= 10;
    let size = if big { 46.0 } else if airy { 30.0 } else { 34.0 };
    let lines = two_lines(&c, f, &card.title, size, max);
    for l in &lines {
        y += size * 1.02;
        c.tl(f, l, size, tx, y, ivory, Weight::Bold, 0.0);
    }
    if let Some(d) = card.detail.as_deref().filter(|s| !s.is_empty()) {
        y += 28.0;
        let d = c.fit(f, d, 21.0, max);
        c.tl(f, &d, 21.0, tx, y.min(H - 34.0), secondary, Weight::Regular, 0.0);
    }
    // progress track + note
    let bottom = H - 22.0;
    if airy {
        // Airy restores short metadata: the note on its own line, times under the track.
        let mut yy = y + 26.0;
        if let Some(p) = card.progress {
            yy = (yy + 4.0).min(H - 40.0);
            c.rect(tx, yy, max, 6.0, 3.0, line, 1.0);
            c.rect(tx, yy, (max * p.clamp(0.0, 1.0)).max(6.0), 6.0, 3.0, th.progress, 1.0);
            if let (Some(n), Kind::Media) = (card.note.as_deref(), card.kind) {
                let mut parts = n.split(" / ");
                let (l, r) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                c.tl(f, l, 15.0, tx, yy + 26.0, secondary, Weight::Regular, 0.0);
                let rw = c.tw(f, r, 15.0, Weight::Regular, 0.0);
                c.tl(f, r, 15.0, tx + max - rw, yy + 26.0, secondary, Weight::Regular, 0.0);
            } else if let Some(n) = card.note.as_deref() {
                let n = c.fit(f, n, 15.0, max);
                c.tl(f, &n, 15.0, tx, yy + 26.0, secondary, Weight::Regular, 0.0);
            }
        } else if let Some(n) = card.note.as_deref() {
            let n = c.fit(f, n, 15.0, max);
            c.tl(f, &n, 15.0, tx, yy.min(H - 18.0), secondary, Weight::Regular, 0.0);
        }
    } else if let Some(p) = card.progress {
        let (px, pw) = (tx, if card.note.is_some() && card.kind == Kind::Media { max - 92.0 } else { max });
        c.rect(px, bottom - 3.0, pw, 6.0, 3.0, line, 1.0);
        c.rect(px, bottom - 3.0, (pw * p.clamp(0.0, 1.0)).max(6.0), 6.0, 3.0, th.progress, 1.0);
        if let Some(n) = card.note.as_deref().filter(|_| card.kind == Kind::Media) {
            let nw = c.tw(f, n, 15.0, Weight::Regular, 0.0);
            c.tl(f, n, 15.0, W - INSET - nw, bottom + 5.0, secondary, Weight::Regular, 0.0);
        } else if let Some(n) = card.note.as_deref() {
            let nw = c.tw(f, n, 15.0, Weight::Regular, 0.0);
            c.tl(f, n, 15.0, W - INSET - nw, bottom - 14.0, ivory, Weight::Regular, 0.0);
        }
    } else if let Some(n) = card.note.as_deref() {
        let nw = c.tw(f, n, 16.0, Weight::Regular, 0.0);
        c.tl(f, n, 16.0, W - INSET - nw, bottom + 4.0, ivory, Weight::Regular, 0.0);
    }
    let _ = secondary;
    if card.critical {
        c.caps_at(f, "Press to dismiss", 11.0, tx, H - 10.0, MUTED);
    }
    // countdown hairline for timed cards
    let dur = card.duration();
    if dur.is_finite() && t < dur {
        c.rect(0.0, H - 2.0, W * (1.0 - t / dur), 2.0, 0.0, mix(accent, BG, 0.4), 1.0);
    }
    if a < 1.0 {
        // fade in from the background
        let mut out = Canvas::new(W as u32, H as u32, th.bg);
        let paint = tiny_skia::PixmapPaint { opacity: a, ..Default::default() };
        out.pm.draw_pixmap(0, 0, c.pm.as_ref(), &paint, tiny_skia::Transform::identity(), None);
        return out;
    }
    c
}

fn default_icon(card: &Card) -> &'static str {
    match card.kind {
        Kind::Email => "outlook",
        Kind::Meeting => "calendar",
        Kind::Printer => "bambu-x1",
        Kind::Steam => "steam",
        Kind::Media => "spotify",
        Kind::Push => {
            if card.critical {
                "warning"
            } else {
                "notification"
            }
        }
    }
}

/// "1:24 / 4:03"
pub fn mmss(pos: f64, dur: f64) -> String {
    let f = |s: f64| {
        let s = s.max(0.0) as u64;
        format!("{}:{:02}", s / 60, s % 60)
    };
    format!("{} / {}", f(pos), f(dur))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priorities() {
        let c = |kind, critical| Card { kind, critical, ..Default::default() };
        assert!(c(Kind::Push, true).priority() > c(Kind::Meeting, false).priority());
        assert!(c(Kind::Meeting, false).priority() > c(Kind::Printer, false).priority());
        assert!(c(Kind::Printer, false).priority() > c(Kind::Push, false).priority());
        assert!(c(Kind::Push, false).priority() > c(Kind::Email, false).priority());
        assert!(c(Kind::Push, true).duration().is_infinite());
        assert_eq!(c(Kind::Email, false).duration(), 8.0);
        assert_eq!(mmss(84.0, 243.0), "1:24 / 4:03");
    }
}
