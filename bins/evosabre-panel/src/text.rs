//! Text on the glass, in whichever face can draw it.
//!
//! The panel's own faces are fixed-width Latin-1. A line that keeps within
//! them is drawn in the face its place on the screen calls for; a line with
//! anything else in it is drawn whole in GNU Unifont, sixteen pixels high,
//! so a title in Japanese or Greek reads as one line of one face.

use crate::frame::Frame;
use crate::unifont::{self, Unifont};
use embedded_graphics::mono_font::{MonoFont, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::{Baseline, Text};

/// A line as the panel can draw it: typographic quotes, dashes and spaces
/// as their plain forms, so a curly apostrophe does not take a Latin title
/// out of its face, and nothing that has no width at all.
pub fn tidy(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{2032}' => out.push('\''),
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{2033}' => out.push('"'),
            '\u{2010}'..='\u{2015}' | '\u{2212}' => out.push('-'),
            '\u{2026}' => out.push_str("..."),
            '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{3000}' | '\t' | '\n' => out.push(' '),
            '\u{200B}'..='\u{200F}' | '\u{2060}' | '\u{FEFF}' => {}
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

/// True when every character is in the panel's own faces.
pub fn latin(s: &str) -> bool {
    s.chars()
        .all(|c| matches!(c as u32, 0x20..=0x7E | 0xA0..=0xFF))
}

#[derive(Clone, Copy)]
pub enum Face {
    /// One of the panel's own fixed-width faces.
    Mono(&'static MonoFont<'static>),
    /// GNU Unifont.
    Wide,
}

impl Face {
    /// The face for a line: `mono` when every character is in it, and when
    /// the wide font cannot be had; the wide font otherwise.
    pub fn choose(s: &str, mono: &'static MonoFont<'static>, font: &mut Unifont) -> Face {
        if latin(s) || !font.usable() {
            Face::Mono(mono)
        } else {
            Face::Wide
        }
    }

    pub fn height(self) -> u32 {
        match self {
            Face::Mono(font) => font.character_size.height,
            Face::Wide => unifont::HEIGHT,
        }
    }

    /// The width of a line in this face, in pixels.
    pub fn width(self, s: &str, font: &mut Unifont) -> u32 {
        match self {
            Face::Mono(mono) => s.chars().count() as u32 * mono.character_size.width,
            Face::Wide => font.width(s),
        }
    }

    /// Draw a line with its top at row `y`, moved left by `offset`, and
    /// cut to the first `width` columns.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        self,
        f: &mut Frame,
        font: &mut Unifont,
        s: &str,
        y: i32,
        luma: u8,
        offset: i32,
        width: u32,
    ) {
        match self {
            Face::Mono(mono) => {
                let rows = Size::new(width, mono.character_size.height);
                let mut area = f.clipped(&Rectangle::new(Point::new(0, y), rows));
                Text::with_baseline(
                    s,
                    Point::new(-offset, y),
                    MonoTextStyle::new(mono, Gray4::new(luma)),
                    Baseline::Top,
                )
                .draw(&mut area)
                .ok();
            }
            Face::Wide => wide(f, font, s, y, luma, offset, width),
        }
    }
}

fn wide(f: &mut Frame, font: &mut Unifont, s: &str, y: i32, luma: u8, offset: i32, width: u32) {
    let limit = width as i32;
    let mut x = -offset;
    for ch in s.chars() {
        if x >= limit {
            break;
        }
        let glyph = font.glyph(ch);
        let advance = glyph.map_or(unifont::MISSING, |g| g.width) as i32;
        if let Some(glyph) = glyph.filter(|_| x + advance > 0) {
            for (row, bits) in glyph.rows.iter().enumerate() {
                let py = y + row as i32;
                if py < 0 {
                    continue;
                }
                for col in 0..glyph.width as i32 {
                    let px = x + col;
                    if bits & (0x8000 >> col) != 0 && px >= 0 && px < limit {
                        f.set(px as u32, py as u32, luma);
                    }
                }
            }
        }
        x += advance;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unifont::tests::sample;
    use embedded_graphics::mono_font::iso_8859_1::FONT_9X18_BOLD;

    #[test]
    fn typographic_marks_become_their_plain_forms_and_the_unseen_go() {
        assert_eq!(tidy("Don\u{2019}t \u{201C}Stop\u{201D}"), "Don't \"Stop\"");
        assert_eq!(tidy("A\u{2013}B\u{2014}C\u{2026}"), "A-B-C...");
        assert_eq!(tidy("a\u{200B}b\u{FEFF}c\u{0007}"), "abc");
        assert_eq!(tidy("one\ttwo\nthree\u{3000}four"), "one two three four");
        assert_eq!(
            tidy("Beyonc\u{E9} \u{6771}\u{4EAC}"),
            "Beyonc\u{E9} \u{6771}\u{4EAC}"
        );
    }

    #[test]
    fn latin_one_is_the_panels_own_and_anything_beyond_is_not() {
        assert!(latin("So Far Away"));
        assert!(latin("Bj\u{F6}rk \u{2022}".trim_end_matches('\u{2022}')));
        assert!(latin("Sigur R\u{F3}s \u{A9} 2005"));
        assert!(!latin("\u{6771}\u{4EAC}"));
        assert!(!latin("Sigur R\u{F3}s \u{2022} Takk"));
        assert!(latin(""));
    }

    #[test]
    fn a_line_keeps_its_own_face_until_it_needs_the_wide_one() {
        let path = sample("face");
        let mut font = Unifont::new(&path);
        assert!(matches!(
            Face::choose("So Far Away", &FONT_9X18_BOLD, &mut font),
            Face::Mono(_)
        ));
        let face = Face::choose("A\u{4E2D}", &FONT_9X18_BOLD, &mut font);
        assert!(matches!(face, Face::Wide));
        assert_eq!(face.width("A\u{4E2D}", &mut font), 24);
        assert_eq!(face.height(), 16);
        let mono = Face::Mono(&FONT_9X18_BOLD);
        assert_eq!(mono.width("So Far Away", &mut font), 99);
        assert_eq!(mono.height(), 18);

        let mut none = Unifont::new("/nonexistent/unifont.hex");
        assert!(
            matches!(
                Face::choose("A\u{4E2D}", &FONT_9X18_BOLD, &mut none),
                Face::Mono(_)
            ),
            "without the wide font the line stays in its own face"
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_wide_line_is_drawn_where_asked_and_cut_at_the_edge() {
        let path = sample("draw");
        let mut font = Unifont::new(&path);
        let mut f = Frame::new();
        // 中 has a full row of pixels at its fourth row, fifteen wide.
        Face::Wide.draw(&mut f, &mut font, "\u{4E2D}", 20, 9, 0, 255);
        assert!((0..15).all(|x| f.get(x, 23) == 9));
        assert_eq!(f.get(15, 23), 0);
        assert_eq!(f.get(7, 20), 9, "the upright");

        let mut f = Frame::new();
        Face::Wide.draw(&mut f, &mut font, "\u{4E2D}", 20, 9, 10, 255);
        assert!((0..5).all(|x| f.get(x, 23) == 9), "moved ten to the left");
        assert_eq!(f.get(5, 23), 0);

        let mut f = Frame::new();
        Face::Wide.draw(&mut f, &mut font, "\u{4E2D}\u{4E2D}", 20, 9, 0, 20);
        assert_eq!(f.get(19, 23), 9);
        assert_eq!(f.get(20, 23), 0, "cut at the width given");
        std::fs::remove_file(&path).unwrap();
    }
}
