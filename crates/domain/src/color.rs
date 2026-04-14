// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Pure color manipulation — blending, HSV conversion, and distance.
//! Zero dependencies. Used by theme adapters to compute tinted surface colors.

/// Linearly blend two RGB colors: `mix(base, overlay, factor)`.
/// `factor` 0.0 = pure base, 1.0 = pure overlay.
/// Returns `#rrggbb` hex string.
pub fn blend(base: (u8, u8, u8), overlay: (u8, u8, u8), factor: f32) -> String {
    let mix = |b: u8, a: u8| -> u8 {
        let v = (b as f32) * (1.0 - factor) + (a as f32) * factor;
        (v.round().clamp(0.0, 255.0)) as u8
    };
    format!(
        "#{:02x}{:02x}{:02x}",
        mix(base.0, overlay.0),
        mix(base.1, overlay.1),
        mix(base.2, overlay.2),
    )
}

/// Convert RGB to HSV. Returns (hue 0–360, saturation 0–255, value 0–255).
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (u16, u8, u8) {
    let (rf, gf, bf) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let delta = max - min;

    let h = if delta < 0.001 {
        0.0
    } else if (max - rf).abs() < 0.001 {
        60.0 * (((gf - bf) / delta) % 6.0)
    } else if (max - gf).abs() < 0.001 {
        60.0 * (((bf - rf) / delta) + 2.0)
    } else {
        60.0 * (((rf - gf) / delta) + 4.0)
    };
    let h = if h < 0.0 { h + 360.0 } else { h };
    let s = if max < 0.001 { 0.0 } else { delta / max };

    (h as u16, (s * 255.0) as u8, (max * 255.0) as u8)
}

/// Convert HSV to RGB.
pub fn hsv_to_rgb(h: u16, s: u8, v: u8) -> (u8, u8, u8) {
    let (sf, vf) = (s as f32 / 255.0, v as f32 / 255.0);
    let hf = h as f32;
    let c = vf * sf;
    let x = c * (1.0 - ((hf / 60.0) % 2.0 - 1.0).abs());
    let m = vf - c;

    let (r, g, b) = match h {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Squared RGB distance between two colors (for nearest-color matching).
pub fn color_distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> u32 {
    let dr = (a.0 as i32 - b.0 as i32).unsigned_abs();
    let dg = (a.1 as i32 - b.1 as i32).unsigned_abs();
    let db = (a.2 as i32 - b.2 as i32).unsigned_abs();
    dr * dr + dg * dg + db * db
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_pure_base() {
        assert_eq!(blend((255, 0, 0), (0, 0, 255), 0.0), "#ff0000");
    }

    #[test]
    fn blend_pure_overlay() {
        assert_eq!(blend((255, 0, 0), (0, 0, 255), 1.0), "#0000ff");
    }

    #[test]
    fn blend_midpoint() {
        let result = blend((0, 0, 0), (100, 100, 100), 0.5);
        assert_eq!(result, "#323232");
    }

    #[test]
    fn hsv_roundtrip() {
        let (h, s, v) = rgb_to_hsv(53, 132, 228); // GNOME blue
        let (r, g, b) = hsv_to_rgb(h, s, v);
        assert!((r as i16 - 53).unsigned_abs() <= 4);
        assert!((g as i16 - 132).unsigned_abs() <= 4);
        assert!((b as i16 - 228).unsigned_abs() <= 4);
    }
}
