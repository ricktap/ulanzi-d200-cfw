//! Run-length coded RGB888 (`IMGR`, `BLITR`): lossless, about 12x smaller than raw for
//! Night Signal keys (flat backgrounds, text, line icons). A control byte `c`:
//! `0..=127`: `c + 1` pixels follow as they are; `128..=255`: one pixel follows and is
//! repeated `c - 126` times (2..=129). The same scheme d200-unchained uses for RGB565.

/// Decode into exactly `pixels` RGB888 pixels.
pub fn decode(src: &[u8], pixels: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(pixels * 3);
    let mut i = 0;
    while i < src.len() {
        let c = src[i] as usize;
        i += 1;
        let (count, literal) = if c < 128 { (c + 1, true) } else { (c - 126, false) };
        let need = if literal { count * 3 } else { 3 };
        if i + need > src.len() {
            return Err("RLE data ends inside a run".into());
        }
        if out.len() + count * 3 > pixels * 3 {
            return Err(format!("RLE data has more than {pixels} pixels"));
        }
        if literal {
            out.extend_from_slice(&src[i..i + need]);
        } else {
            for _ in 0..count {
                out.extend_from_slice(&src[i..i + 3]);
            }
        }
        i += need;
    }
    if out.len() != pixels * 3 {
        return Err(format!("RLE data has {} of {pixels} pixels", out.len() / 3));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn runs() {
        // two literal pixels, then one repeated 3 times
        let src = [1, 1, 2, 3, 4, 5, 6, 129, 9, 9, 9];
        assert_eq!(decode(&src, 5).unwrap(), [1, 2, 3, 4, 5, 6, 9, 9, 9, 9, 9, 9, 9, 9, 9]);
        assert!(decode(&src, 4).is_err());
        assert!(decode(&[129, 9, 9], 3).is_err(), "truncated run");
    }
}
