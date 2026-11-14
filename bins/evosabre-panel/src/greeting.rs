//! The greeting: what the panel shows from power-on until the player answers.
//!
//! The logo replaces the greeting text when one is present. The logo is a
//! PGM (P5) of exactly the panel's size; eight-bit grey maps onto the
//! panel's sixteen levels, so a mark drawn with soft edges keeps them. The
//! one the plugin ships is drawn by `scripts/greeting.sh`; any image becomes
//! one with ImageMagick:
//!
//! ```text
//! magick logo.png -resize 256x64 -gravity center -background black \
//!     -extent 256x64 -colorspace gray -depth 8 logo.pgm
//! ```
//!
//! A logo that cannot be read or is the wrong shape is reported, and the
//! text is shown instead: the panel never stays dark over a bad file.

use crate::config::Config;
use crate::frame::{self, Frame};
use embedded_graphics::mono_font::iso_8859_1::FONT_10X20;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Alignment, Baseline, Text, TextStyleBuilder};

/// The frame to show and a line for the journal saying where it came from.
pub fn load(cfg: &Config) -> (Frame, String) {
    match std::fs::read(&cfg.logo_path) {
        Ok(bytes) => match parse_pgm(&bytes) {
            Ok(frame) => (frame, format!("logo {}", cfg.logo_path)),
            Err(e) => (
                text(&cfg.greeting_text),
                format!(
                    "logo {} not usable ({e}); showing the greeting text",
                    cfg.logo_path
                ),
            ),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (
            text(&cfg.greeting_text),
            format!("no logo at {}; showing the greeting text", cfg.logo_path),
        ),
        Err(e) => (
            text(&cfg.greeting_text),
            format!(
                "logo {} not readable ({e}); showing the greeting text",
                cfg.logo_path
            ),
        ),
    }
}

/// The greeting text, centred, in the large face.
pub fn text(greeting: &str) -> Frame {
    let mut f = Frame::new();
    let style = MonoTextStyle::new(&FONT_10X20, Gray4::WHITE);
    let layout = TextStyleBuilder::new()
        .alignment(Alignment::Center)
        .baseline(Baseline::Middle)
        .build();
    Text::with_text_style(
        greeting,
        Point::new(frame::WIDTH as i32 / 2, frame::HEIGHT as i32 / 2),
        style,
        layout,
    )
    .draw(&mut f)
    .ok();
    f
}

/// Decode a binary PGM of the panel's size into a frame.
pub fn parse_pgm(bytes: &[u8]) -> Result<Frame, String> {
    let mut cursor = 0;
    let magic = token(bytes, &mut cursor).ok_or("empty file")?;
    if magic != b"P5" {
        return Err(format!(
            "not a binary PGM (P5); starts with {:?}",
            String::from_utf8_lossy(magic)
        ));
    }
    let mut dims = [0u32; 3];
    for (i, name) in ["width", "height", "maxval"].iter().enumerate() {
        let t = token(bytes, &mut cursor).ok_or(format!("{name} missing"))?;
        dims[i] = std::str::from_utf8(t)
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or(format!("{name} is not a number"))?;
    }
    let [width, height, maxval] = dims;
    if width != frame::WIDTH || height != frame::HEIGHT {
        return Err(format!(
            "{width}x{height}; the panel is {}x{}",
            frame::WIDTH,
            frame::HEIGHT
        ));
    }
    if maxval == 0 || maxval > 255 {
        return Err(format!("maxval {maxval}; must be 1-255"));
    }
    // One whitespace byte separates the header from the raster.
    cursor += 1;
    let raster = bytes
        .get(cursor..cursor + frame::WIDTH as usize * frame::HEIGHT as usize)
        .ok_or("raster is short")?;

    let mut f = Frame::new();
    for (i, &v) in raster.iter().enumerate() {
        let luma = ((v as u32 * 15 + maxval / 2) / maxval) as u8;
        f.set(i as u32 % frame::WIDTH, i as u32 / frame::WIDTH, luma);
    }
    Ok(f)
}

/// The next header token: whitespace and `#` comments skipped.
fn token<'a>(bytes: &'a [u8], cursor: &mut usize) -> Option<&'a [u8]> {
    loop {
        while *cursor < bytes.len() && bytes[*cursor].is_ascii_whitespace() {
            *cursor += 1;
        }
        if *cursor < bytes.len() && bytes[*cursor] == b'#' {
            while *cursor < bytes.len() && bytes[*cursor] != b'\n' {
                *cursor += 1;
            }
            continue;
        }
        break;
    }
    let start = *cursor;
    while *cursor < bytes.len() && !bytes[*cursor].is_ascii_whitespace() {
        *cursor += 1;
    }
    (*cursor > start).then(|| &bytes[start..*cursor])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pgm(width: u32, height: u32, maxval: u32, header_comment: bool) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"P5\n");
        if header_comment {
            v.extend_from_slice(b"# made for the test\n");
        }
        v.extend_from_slice(format!("{width} {height}\n{maxval}\n").as_bytes());
        for y in 0..height {
            for x in 0..width {
                v.push(((x + y) % (maxval + 1)) as u8);
            }
        }
        v
    }

    #[test]
    fn a_panel_sized_pgm_decodes_with_eight_bit_grey_onto_sixteen_levels() {
        let f = parse_pgm(&pgm(256, 64, 255, true)).unwrap();
        assert_eq!(f.get(0, 0), 0);
        assert_eq!(f.get(255, 0), 15);
        assert_eq!(f.get(128, 0), ((128u32 * 15 + 127) / 255) as u8);
    }

    #[test]
    fn a_smaller_maxval_scales_too() {
        let f = parse_pgm(&pgm(256, 64, 15, false)).unwrap();
        assert_eq!(f.get(15, 0), 15);
        assert_eq!(f.get(7, 0), 7);
    }

    #[test]
    fn the_wrong_shape_is_refused_with_the_shape_named() {
        let e = parse_pgm(&pgm(128, 64, 255, false)).unwrap_err();
        assert!(e.contains("128x64"), "{e}");
    }

    #[test]
    fn other_formats_and_short_files_are_refused() {
        assert!(parse_pgm(b"P2\n256 64\n255\n").unwrap_err().contains("P5"));
        assert!(parse_pgm(b"P5\n256 64\n65535\n")
            .unwrap_err()
            .contains("maxval"));
        let mut short = pgm(256, 64, 255, false);
        short.truncate(short.len() - 1);
        assert!(parse_pgm(&short).unwrap_err().contains("short"));
        assert!(parse_pgm(b"").is_err());
    }

    #[test]
    fn the_text_greeting_is_centred_and_lit() {
        let f = text("EVO SABRE");
        assert!(f.is_lit());
        let lit_x: Vec<u32> = (0..frame::WIDTH)
            .filter(|&x| (0..frame::HEIGHT).any(|y| f.get(x, y) > 0))
            .collect();
        let (left, right) = (*lit_x.first().unwrap(), *lit_x.last().unwrap());
        assert!(left < 128 && right > 128, "{left}..{right}");
        assert!(
            (left as i32 - (255 - right as i32)).abs() <= 10,
            "{left}..{right}"
        );
    }

    #[test]
    fn load_falls_back_to_text_and_says_why() {
        let cfg = Config {
            logo_path: "/nonexistent/evosabre/logo.pgm".into(),
            ..Config::default()
        };
        let (f, why) = load(&cfg);
        assert!(f.is_lit());
        assert!(why.starts_with("no logo at"), "{why}");
    }
}
