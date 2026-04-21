// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Wallpaper palette extraction — shared between the daemon and the UI.
//!
//! Handles two wallpaper shapes:
//!
//! - A single image file (jpg / png / webp / ...) — loaded directly.
//! - A GNOME slideshow XML (`<background>` with `<static>`/`<transition>`
//!   children) — we pick the picture that's *currently* showing based on
//!   wall-clock time and extract from that.
//!
//! Before this module landed, both the `experienced` daemon and the
//! Theme Builder UI had their own near-identical extractor that called
//! `Pixbuf::from_file_at_scale` straight on `picture-uri`. When the
//! wallpaper slideshow feature (GXF-072) started writing `.xml` files
//! into `picture-uri`, both extractors failed silently because pixbuf
//! cannot parse XML. This module is the single place that knows about
//! both shapes.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use gdk_pixbuf::Pixbuf;
use gnomex_domain::{color, HexColor};

/// GNOME accent-color id / hex swatch pairs. The *ids* are what we
/// write back into `org.gnome.desktop.interface accent-color`.
pub const ACCENT_COLORS: &[(&str, &str)] = &[
    ("blue", "#3584e4"),
    ("teal", "#2190a4"),
    ("green", "#3a944a"),
    ("yellow", "#c88800"),
    ("orange", "#ed5b00"),
    ("red", "#e62d42"),
    ("pink", "#d56199"),
    ("purple", "#9141ac"),
    ("slate", "#6f8396"),
];

/// Maximum palette entries returned by [`extract_palette`].
pub const MAX_PALETTE: usize = 5;

/// Resolve a value that the user gave us — a `file://` URI, a bare
/// absolute path, or a slideshow XML path — into the concrete image
/// file we should sample pixels from *right now*.
///
/// Returns `Ok(Some(path))` when an image was located, `Ok(None)` when
/// the path is a slideshow XML that has no `<static>` entry we can
/// resolve, and `Err` on IO failure (but NOT when the XML simply isn't
/// a slideshow — that degrades to "treat the path as an image").
pub fn resolve_wallpaper_image(raw: &str) -> Result<Option<PathBuf>> {
    let bare = raw.strip_prefix("file://").unwrap_or(raw);
    let path = Path::new(bare);
    if path.extension().and_then(|s| s.to_str()) != Some("xml") {
        return Ok(Some(path.to_path_buf()));
    }

    let xml = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => return Err(e).with_context(|| format!("read slideshow XML: {bare}")),
    };
    Ok(current_slideshow_image(&xml, path.parent(), now_unix()))
}

/// Pick the slideshow entry that should be showing at `now_epoch`
/// (seconds since Unix epoch) for a slideshow XML whose content is
/// `xml`. `relative_to` is the slideshow file's parent directory, used
/// to resolve relative `<file>` paths.
///
/// Returns the absolute path of the currently-active image, or `None`
/// if the XML has no `<static>` entries at all.
///
/// The algorithm:
/// 1. Parse all `<static><duration><file>` entries in document order.
/// 2. Ignore `<starttime>` — GNOME respects it for exact phase, but for
///    a colour extractor it's enough to pick the entry at `now % sum`
///    since the dominant hues stabilise across a cycle.
/// 3. Project `now_epoch` onto the total loop length; walk the static
///    entries until we land inside one.
///
/// This does *not* try to honour `<transition>` — during a transition
/// GNOME is cross-fading, but both endpoints are already in the
/// `<static>` list we'll sample, and the crossfade window is much
/// shorter than the static hold.
pub fn current_slideshow_image(
    xml: &str,
    relative_to: Option<&Path>,
    now_epoch: u64,
) -> Option<PathBuf> {
    let statics = parse_static_entries(xml);
    if statics.is_empty() {
        return None;
    }
    let total: f64 = statics.iter().map(|(d, _)| d.max(0.0)).sum();
    if total <= 0.0 {
        return statics.into_iter().next().map(|(_, p)| resolve_rel(relative_to, p));
    }

    let phase = (now_epoch as f64) % total;
    let mut acc = 0.0f64;
    for (dur, file) in statics {
        let seg = dur.max(0.0);
        if phase < acc + seg {
            return Some(resolve_rel(relative_to, file));
        }
        acc += seg;
    }
    // Floating-point rounding edge — fall back to the last entry.
    None
}

fn resolve_rel(relative_to: Option<&Path>, file: String) -> PathBuf {
    let p = Path::new(&file);
    if p.is_absolute() {
        p.to_path_buf()
    } else if let Some(base) = relative_to {
        base.join(p)
    } else {
        p.to_path_buf()
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Cheap `<static>` parser — returns `(duration_seconds, file)` tuples
/// in document order. The slideshow format is small enough that a
/// dedicated parser dependency would be overkill; this walks tag pairs
/// by hand. Case-sensitive (GNOME writes them lowercase).
fn parse_static_entries(xml: &str) -> Vec<(f64, String)> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<static>") {
        let after = &rest[start + "<static>".len()..];
        let Some(end) = after.find("</static>") else {
            break;
        };
        let body = &after[..end];

        let duration = extract_tag(body, "duration")
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        if let Some(file) = extract_tag(body, "file") {
            let file = xml_unescape(file.trim());
            out.push((duration, file));
        }

        rest = &after[end + "</static>".len()..];
    }
    out
}

fn extract_tag<'a>(body: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let i = body.find(&open)?;
    let after = &body[i + open.len()..];
    let j = after.find(&close)?;
    Some(&after[..j])
}

fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
}

/// A single palette entry: a candidate hex plus the nearest GNOME
/// accent id it was mapped to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    pub hex: String,
    pub accent_id: String,
}

/// Extract up to [`MAX_PALETTE`] distinct candidate colours from
/// `image_path`. Returns empty if the image loads but has no
/// sufficiently-saturated pixels.
pub fn extract_palette(image_path: &Path) -> Result<Vec<PaletteEntry>> {
    let pixbuf = Pixbuf::from_file_at_scale(
        image_path.to_str().context("non-utf8 image path")?,
        64,
        64,
        true,
    )
    .with_context(|| format!("load wallpaper image: {}", image_path.display()))?;
    let Some(pixels) = pixbuf.pixel_bytes() else {
        return Ok(Vec::new());
    };
    let channels = pixbuf.n_channels() as usize;
    let data = pixels.as_ref();

    let mut buckets: std::collections::HashMap<(u16, u8, u8), u32> = std::collections::HashMap::new();
    for chunk in data.chunks_exact(channels) {
        if channels < 3 {
            continue;
        }
        let (r, g, b) = (chunk[0], chunk[1], chunk[2]);
        let (h, s, v) = color::rgb_to_hsv(r, g, b);
        if s < 15 || v < 25 || v > 240 {
            continue;
        }
        let hue_bin = (h / 10) * 10;
        let sat_bin = (s / 85).min(2);
        let val_bin = (v / 85).min(2);
        *buckets.entry((hue_bin, sat_bin, val_bin)).or_default() += 1;
    }

    let mut sorted: Vec<_> = buckets.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let mut results: Vec<PaletteEntry> = Vec::with_capacity(MAX_PALETTE);
    let mut seen_accents: std::collections::HashSet<String> = std::collections::HashSet::new();
    for ((hue_bin, sat_bin, val_bin), _) in sorted.iter().take(20) {
        let h = *hue_bin + 5;
        let s = (*sat_bin as u16) * 85 + 42;
        let v = (*val_bin as u16) * 85 + 42;
        let (r, g, b) = color::hsv_to_rgb(h, s.min(255) as u8, v.min(255) as u8);
        let accent_id = closest_accent_id(r, g, b);
        if seen_accents.contains(&accent_id) {
            continue;
        }
        seen_accents.insert(accent_id.clone());
        results.push(PaletteEntry {
            hex: format!("#{r:02x}{g:02x}{b:02x}"),
            accent_id,
        });
        if results.len() >= MAX_PALETTE {
            break;
        }
    }
    Ok(results)
}

/// Nearest GNOME accent id for an (r, g, b) triple.
pub fn closest_accent_id(r: u8, g: u8, b: u8) -> String {
    let mut best = "blue";
    let mut best_dist = u32::MAX;
    for &(id, hex) in ACCENT_COLORS {
        let accent = HexColor::new(hex).map(|c| c.to_rgb()).unwrap_or((0, 0, 0));
        let dist = color::color_distance((r, g, b), accent);
        if dist < best_dist {
            best_dist = dist;
            best = id;
        }
    }
    best.to_owned()
}

/// Pick the accent id from `palette` that the user has locked to, or
/// fall back to the dominant (index 0) when the lock is `-1` or the
/// palette has fewer entries than the lock. Returns `None` when the
/// palette is empty.
///
/// This is the core of the "remember which swatch the user picked"
/// behaviour — see `wallpaper-accent-index` in the GSettings schema.
pub fn locked_accent(palette: &[PaletteEntry], locked_index: i32) -> Option<&str> {
    if palette.is_empty() {
        return None;
    }
    let idx = if locked_index >= 0 && (locked_index as usize) < palette.len() {
        locked_index as usize
    } else {
        0
    };
    Some(&palette[idx].accent_id)
}

/// Encode a palette for storage in the `cached-wallpaper-palette`
/// strv GSetting. Each entry is `"HEX:ACCENT_ID"`.
pub fn encode_palette(palette: &[PaletteEntry]) -> Vec<String> {
    palette
        .iter()
        .map(|p| format!("{}:{}", p.hex, p.accent_id))
        .collect()
}

/// Parse the `cached-wallpaper-palette` strv back into structured form.
/// Silently drops malformed entries.
pub fn decode_palette<S: AsRef<str>>(entries: &[S]) -> Vec<PaletteEntry> {
    entries
        .iter()
        .filter_map(|e| {
            let (hex, accent) = e.as_ref().split_once(':')?;
            if hex.is_empty() || accent.is_empty() {
                None
            } else {
                Some(PaletteEntry {
                    hex: hex.to_owned(),
                    accent_id: accent.to_owned(),
                })
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- Generated by GNOME X -->
<background>
  <starttime>
    <year>2026</year><month>1</month><day>1</day>
    <hour>0</hour><minute>0</minute><second>0</second>
  </starttime>
  <static><duration>10.0</duration><file>/abs/a.jpg</file></static>
  <transition type="overlay"><duration>1.0</duration><from>/abs/a.jpg</from><to>/abs/b.jpg</to></transition>
  <static><duration>10.0</duration><file>/abs/b.jpg</file></static>
  <transition type="overlay"><duration>1.0</duration><from>/abs/b.jpg</from><to>/abs/a.jpg</to></transition>
</background>
"#;

    #[test]
    fn parse_extracts_both_statics() {
        let statics = parse_static_entries(SAMPLE_XML);
        assert_eq!(statics.len(), 2);
        assert_eq!(statics[0].0, 10.0);
        assert_eq!(statics[0].1, "/abs/a.jpg");
        assert_eq!(statics[1].1, "/abs/b.jpg");
    }

    #[test]
    fn current_image_picks_first_at_t_equals_zero() {
        let got = current_slideshow_image(SAMPLE_XML, None, 0).unwrap();
        assert_eq!(got, Path::new("/abs/a.jpg"));
    }

    #[test]
    fn current_image_wraps_modulo_total_duration() {
        // Total cycle ~ 20s (the 1s transitions are ignored by the
        // static walker — they're crossfades, not holds). At t=12s
        // we're inside the SECOND static (a=0..10, b=10..20).
        let got = current_slideshow_image(SAMPLE_XML, None, 12).unwrap();
        assert_eq!(got, Path::new("/abs/b.jpg"));

        // At t=22s = 2 into next cycle → back to first.
        let got = current_slideshow_image(SAMPLE_XML, None, 22).unwrap();
        assert_eq!(got, Path::new("/abs/a.jpg"));
    }

    #[test]
    fn relative_file_resolves_against_base() {
        let xml = r#"<background>
            <static><duration>5.0</duration><file>rel.jpg</file></static>
        </background>"#;
        let got = current_slideshow_image(xml, Some(Path::new("/base/dir")), 0).unwrap();
        assert_eq!(got, Path::new("/base/dir/rel.jpg"));
    }

    #[test]
    fn xml_unescape_handles_ampersand_and_quotes() {
        assert_eq!(
            xml_unescape("cats &amp; dogs &quot;rock&quot;"),
            "cats & dogs \"rock\"",
        );
    }

    #[test]
    fn resolve_non_xml_path_returns_path_unchanged() {
        let got = resolve_wallpaper_image("file:///abs/pic.jpg").unwrap().unwrap();
        assert_eq!(got, Path::new("/abs/pic.jpg"));
        let got = resolve_wallpaper_image("/abs/pic.png").unwrap().unwrap();
        assert_eq!(got, Path::new("/abs/pic.png"));
    }

    #[test]
    fn palette_encode_decode_roundtrip() {
        let p = vec![
            PaletteEntry { hex: "#ff0000".into(), accent_id: "red".into() },
            PaletteEntry { hex: "#3584e4".into(), accent_id: "blue".into() },
        ];
        let encoded = encode_palette(&p);
        let decoded = decode_palette(&encoded);
        assert_eq!(decoded, p);
    }

    #[test]
    fn locked_accent_respects_index_when_in_range() {
        let p = vec![
            PaletteEntry { hex: "#ff0000".into(), accent_id: "red".into() },
            PaletteEntry { hex: "#00ff00".into(), accent_id: "green".into() },
            PaletteEntry { hex: "#0000ff".into(), accent_id: "blue".into() },
        ];
        assert_eq!(locked_accent(&p, 1), Some("green"));
        assert_eq!(locked_accent(&p, 2), Some("blue"));
    }

    #[test]
    fn locked_accent_falls_back_to_dominant_when_unlocked_or_oob() {
        let p = vec![
            PaletteEntry { hex: "#ff0000".into(), accent_id: "red".into() },
            PaletteEntry { hex: "#00ff00".into(), accent_id: "green".into() },
        ];
        // -1 = "no lock" → dominant.
        assert_eq!(locked_accent(&p, -1), Some("red"));
        // Out-of-range (user locked to a 5-colour palette, new wallpaper
        // only yielded 2) → dominant.
        assert_eq!(locked_accent(&p, 4), Some("red"));
    }

    #[test]
    fn locked_accent_empty_palette_is_none() {
        assert_eq!(locked_accent(&[], 0), None);
        assert_eq!(locked_accent(&[], -1), None);
    }

    #[test]
    fn decode_silently_drops_malformed_entries() {
        let raw: Vec<String> = vec!["ok:blue".into(), "broken".into(), ":missing".into(), "also:ok".into()];
        let decoded = decode_palette(&raw);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].accent_id, "blue");
        assert_eq!(decoded[1].hex, "also");
    }
}
