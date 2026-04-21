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

// ---------------- CIELAB / perceptual colour distance ----------------
//
// The palette extractor uses LAB because sRGB-Euclidean distance is
// perceptually uneven: colours that look close can be numerically far
// (and vice-versa). LAB's `delta-E` (CIE76) is a better stand-in for
// "how similar do these two colours *look*", which is what we want
// when snapping a wallpaper swatch to GNOME's 9-accent palette.

/// Convert a gamma-encoded sRGB byte to a linear-light [0, 1] float.
fn srgb_to_linear(c: u8) -> f32 {
    let v = c as f32 / 255.0;
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert sRGB → CIE XYZ (D65 illuminant). sRGB matrix values from
/// IEC 61966-2-1.
fn rgb_to_xyz(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = srgb_to_linear(r);
    let g = srgb_to_linear(g);
    let b = srgb_to_linear(b);
    let x = 0.412_390_8 * r + 0.357_584_3 * g + 0.180_480_7 * b;
    let y = 0.212_639_0 * r + 0.715_168_7 * g + 0.072_192_3 * b;
    let z = 0.019_330_8 * r + 0.119_194_8 * g + 0.950_532_1 * b;
    (x, y, z)
}

/// f() mapping used by CIE L*a*b*. Kept private; callers use
/// [`rgb_to_lab`] directly.
fn lab_f(t: f32) -> f32 {
    const DELTA: f32 = 6.0 / 29.0;
    if t > DELTA.powi(3) {
        t.cbrt()
    } else {
        t / (3.0 * DELTA * DELTA) + 4.0 / 29.0
    }
}

/// Convert sRGB to CIELAB under a D65 white point. Returns
/// `(L, a, b)` where L ∈ [0, 100] and a/b ∈ roughly [-128, 127].
pub fn rgb_to_lab(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    // D65 reference white, normalised with Yn = 1.0.
    const XN: f32 = 0.950_47;
    const YN: f32 = 1.000_00;
    const ZN: f32 = 1.088_83;
    let (x, y, z) = rgb_to_xyz(r, g, b);
    let fx = lab_f(x / XN);
    let fy = lab_f(y / YN);
    let fz = lab_f(z / ZN);
    let l = 116.0 * fy - 16.0;
    let a = 500.0 * (fx - fy);
    let bv = 200.0 * (fy - fz);
    (l, a, bv)
}

/// CIE76 ΔE — straight Euclidean distance in L*a*b*. Not the most
/// modern colour-difference metric (that's ΔE2000), but 20× cheaper
/// and "correct enough" for picking between 9 well-spaced accents.
pub fn lab_distance(a: (f32, f32, f32), b: (f32, f32, f32)) -> f32 {
    let dl = a.0 - b.0;
    let da = a.1 - b.1;
    let db = a.2 - b.2;
    (dl * dl + da * da + db * db).sqrt()
}

// ---------------- Nearest GNOME enum accent -----------------------
//
// `org.gnome.desktop.interface accent-color` is an enum; external
// tools (Adwaita's native folder-colour tinting, Papirus-folders)
// all take one of the nine named values. This helper exists in the
// domain (not infra) because Material-palette derivation needs it
// to propagate a derived hex into the stock enum.

/// GNOME's nine stock accents, keyed by their GSettings enum id.
/// Hexes match the palette Adwaita ships for each variant.
pub const GNOME_ACCENTS: &[(&str, (u8, u8, u8))] = &[
    ("blue", (0x35, 0x84, 0xe4)),
    ("teal", (0x21, 0x90, 0xa4)),
    ("green", (0x3a, 0x94, 0x4a)),
    ("yellow", (0xc8, 0x88, 0x00)),
    ("orange", (0xed, 0x5b, 0x00)),
    ("red", (0xe6, 0x2d, 0x42)),
    ("pink", (0xd5, 0x61, 0x99)),
    ("purple", (0x91, 0x41, 0xac)),
    ("slate", (0x6f, 0x83, 0x96)),
];

/// Map an (r, g, b) triple to the nearest GNOME enum accent id by
/// CIE76 ΔE. Same classifier the LAB wallpaper extractor uses; kept
/// public so MD3 can propagate derived primary colours into the
/// stock accent enum without re-implementing the distance metric.
pub fn closest_gnome_accent_id(r: u8, g: u8, b: u8) -> String {
    let candidate = rgb_to_lab(r, g, b);
    let mut best = "blue";
    let mut best_d = f32::MAX;
    for &(id, (ar, ag, ab)) in GNOME_ACCENTS {
        let accent = rgb_to_lab(ar, ag, ab);
        let d = lab_distance(candidate, accent);
        if d < best_d {
            best_d = d;
            best = id;
        }
    }
    best.to_owned()
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
