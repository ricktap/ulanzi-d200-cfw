//! The idle scene. The drawing lives in the `scene` crate, shared with d200fw, which
//! renders it on the device when it can (no frames over USB).

pub use scene::{key_rect, Scene, Season, H, W};

/// A season by name; "auto" (or anything unknown) picks it from the local month.
pub fn parse(s: &str, southern: bool) -> Season {
    Season::from_name(s).unwrap_or_else(|| auto(southern))
}

/// Meteorological season from the local month.
pub fn auto(southern: bool) -> Season {
    use chrono::Datelike;
    let mut m = chrono::Local::now().month0(); // 0 = January
    if southern {
        m = (m + 6) % 12;
    }
    match m {
        11 | 0 | 1 => Season::Snow,
        2..=4 => Season::Pink,
        5..=7 => Season::Beach,
        _ => Season::Leaves,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn season_names() {
        assert_eq!(parse("auto", false), auto(false));
        assert_eq!(parse("rain", true), Season::Rain);
    }
}
