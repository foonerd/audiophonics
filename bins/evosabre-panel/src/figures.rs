//! The figures of the track's time, at four sizes.
//!
//! The smallest two keep the track's three lines above them: the face the
//! time has always had, six rows tall, and one drawn here, eight rows tall,
//! which is as tall as a time can be under an album's line. The larger two
//! take the album's rows: a face of the panel's own, ten rows tall, and
//! seven-segment figures of thirteen, drawn as the resting clock's are.
//!
//! A time is placed by the top row of its figures and by its first lit
//! column, whatever the face, and a space in it is as wide as a figure: a
//! time padded to the length of another is as wide as that other.

use crate::frame::Frame;
use crate::segments;
use embedded_graphics::mono_font::iso_8859_1::{FONT_5X8, FONT_7X14};
use embedded_graphics::mono_font::{MonoFont, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text};

/// How large the time is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Small,
    Medium,
    Large,
    ExtraLarge,
}

#[cfg(test)]
pub const SIZES: [Size; 4] = [Size::Small, Size::Medium, Size::Large, Size::ExtraLarge];

/// A face of the panel's own, and how many rows of its cell stand above
/// the top of its figures.
struct Type {
    font: &'static MonoFont<'static>,
    above: u32,
}

const SMALL: Type = Type {
    font: &FONT_5X8,
    above: 1,
};
const LARGE: Type = Type {
    font: &FONT_7X14,
    above: 2,
};

/// The eight-row figures: five columns each, the colon three, a column
/// between two.
const MEDIUM_ROWS: u32 = 8;
const MEDIUM_W: u32 = 5;
const MEDIUM_COLON_W: u32 = 3;
const MEDIUM: [[&str; 8]; 10] = [
    [
        ".XXX.", "X...X", "X...X", "X...X", "X...X", "X...X", "X...X", ".XXX.",
    ],
    [
        "..X..", ".XX..", "..X..", "..X..", "..X..", "..X..", "..X..", ".XXX.",
    ],
    [
        ".XXX.", "X...X", "....X", "...X.", "..X..", ".X...", "X....", "XXXXX",
    ],
    [
        ".XXX.", "X...X", "....X", "..XX.", "....X", "....X", "X...X", ".XXX.",
    ],
    [
        "...X.", "..XX.", ".X.X.", "X..X.", "X..X.", "XXXXX", "...X.", "...X.",
    ],
    [
        "XXXXX", "X....", "X....", "XXXX.", "....X", "....X", "X...X", ".XXX.",
    ],
    [
        ".XXX.", "X...X", "X....", "XXXX.", "X...X", "X...X", "X...X", ".XXX.",
    ],
    [
        "XXXXX", "....X", "....X", "...X.", "...X.", "..X..", "..X..", "..X..",
    ],
    [
        ".XXX.", "X...X", "X...X", ".XXX.", "X...X", "X...X", "X...X", ".XXX.",
    ],
    [
        ".XXX.", "X...X", "X...X", "X...X", ".XXXX", "....X", "X...X", ".XXX.",
    ],
];
/// The rows of the colon's two dots, in its middle column.
const MEDIUM_COLON: [u32; 2] = [2, 5];

/// The thirteen-row figures: seven segments two pixels thick in six
/// columns, two columns between two figures. Thirteen rows leave an odd
/// number between the top segment and the bottom one, so the middle
/// segment is one row and the two halves match.
const SEGMENT_ROWS: u32 = 13;
const SEGMENT_W: u32 = 6;
const SEGMENT_T: u32 = 2;
const SEGMENT_GAP: u32 = 2;
const SEGMENT_MIDDLE: u32 = 6;
/// The top rows of the colon's two dots.
const SEGMENT_COLON: [u32; 2] = [3, 8];

impl Size {
    /// The size the settings name `s`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "large" => Some(Self::Large),
            "extra_large" => Some(Self::ExtraLarge),
            _ => None,
        }
    }

    /// The rows its figures stand in.
    pub fn rows(self) -> u32 {
        match self {
            Self::Small => 6,
            Self::Medium => MEDIUM_ROWS,
            Self::Large => 10,
            Self::ExtraLarge => SEGMENT_ROWS,
        }
    }

    /// The columns between two of its characters.
    fn between(self) -> u32 {
        match self {
            Self::Small | Self::Medium | Self::Large => 1,
            Self::ExtraLarge => SEGMENT_GAP,
        }
    }

    /// The columns one character is drawn in.
    fn drawn(self, c: char) -> u32 {
        match (self, c) {
            (Self::Small, _) => SMALL.font.character_size.width - 1,
            (Self::Large, _) => LARGE.font.character_size.width - 1,
            (Self::Medium, ':') => MEDIUM_COLON_W,
            (Self::Medium, _) => MEDIUM_W,
            (Self::ExtraLarge, ':') => SEGMENT_T,
            (Self::ExtraLarge, _) => SEGMENT_W,
        }
    }

    /// The columns a time takes, from its first to its last.
    pub fn width(self, time: &str) -> u32 {
        time.chars()
            .map(|c| self.drawn(c) + self.between())
            .sum::<u32>()
            .saturating_sub(self.between())
    }

    /// Draw `time` from column `x`, the top of its figures on row `top`.
    /// The segments of the largest figures that are not lit are drawn at
    /// `unlit`, as the clock's are; the other sizes have none.
    pub fn draw(self, f: &mut Frame, x: u32, top: u32, time: &str, luma: u8, unlit: u8) {
        match self {
            Self::Small => typed(f, &SMALL, x, top, time, luma),
            Self::Large => typed(f, &LARGE, x, top, time, luma),
            Self::Medium | Self::ExtraLarge => {
                let mut cx = x;
                for c in time.chars() {
                    match (self, c.to_digit(10)) {
                        (Self::Medium, Some(d)) => medium(f, cx, top, d as usize, luma),
                        (Self::Medium, None) if c == ':' => {
                            for row in MEDIUM_COLON {
                                f.set(cx + 1, top + row, luma);
                            }
                        }
                        (Self::ExtraLarge, Some(d)) => {
                            seven(f, cx, top, Some(d as u8), luma, unlit)
                        }
                        (Self::ExtraLarge, None) if c == ':' => {
                            for row in SEGMENT_COLON {
                                block(f, cx, top + row, SEGMENT_T, SEGMENT_T, luma);
                            }
                        }
                        // A space: a figure with nothing lit.
                        (Self::ExtraLarge, None) => seven(f, cx, top, None, luma, unlit),
                        _ => {}
                    }
                    cx += self.drawn(c) + self.between();
                }
            }
        }
    }
}

/// A time in one of the panel's own faces.
fn typed(f: &mut Frame, face: &Type, x: u32, top: u32, time: &str, luma: u8) {
    let y = top as i32 - face.above as i32;
    Text::with_baseline(
        time,
        Point::new(x as i32, y),
        MonoTextStyle::new(face.font, Gray4::new(luma)),
        Baseline::Top,
    )
    .draw(f)
    .ok();
}

fn medium(f: &mut Frame, x: u32, top: u32, digit: usize, luma: u8) {
    for (row, line) in MEDIUM[digit].iter().enumerate() {
        for (column, c) in line.bytes().enumerate() {
            if c == b'X' {
                f.set(x + column as u32, top + row as u32, luma);
            }
        }
    }
}

fn block(f: &mut Frame, x: u32, y: u32, w: u32, h: u32, luma: u8) {
    for dy in 0..h {
        for dx in 0..w {
            f.set(x + dx, y + dy, luma);
        }
    }
}

/// One seven-segment figure, or with `None` its seven segments unlit.
fn seven(f: &mut Frame, x: u32, y: u32, digit: Option<u8>, lit: u8, unlit: u8) {
    let (w, h, t) = (SEGMENT_W, SEGMENT_ROWS, SEGMENT_T);
    let mid = SEGMENT_MIDDLE;
    let mask = digit.map_or(0, |d| segments::MASKS[usize::from(d.min(9))]);
    // Segments a to g, as the clock's: each its left column, its top row,
    // its width and its height.
    let boxes = [
        (1, 0, w - 2, t),
        (w - t, t, t, mid - t),
        (w - t, mid + 1, t, h - t - mid - 1),
        (1, h - t, w - 2, t),
        (0, mid + 1, t, h - t - mid - 1),
        (0, t, t, mid - t),
        (1, mid, w - 2, 1),
    ];
    for (bit, (bx, by, bw, bh)) in boxes.into_iter().enumerate() {
        let on = mask & (1 << bit) != 0;
        if on || unlit > 0 {
            block(f, x + bx, y + by, bw, bh, if on { lit } else { unlit });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame;

    /// The rows and the columns that carry anything.
    fn extent(f: &Frame) -> (Vec<u32>, Vec<u32>) {
        let rows = (0..frame::HEIGHT)
            .filter(|&y| (0..frame::WIDTH).any(|x| f.get(x, y) != 0))
            .collect();
        let columns = (0..frame::WIDTH)
            .filter(|&x| (0..frame::HEIGHT).any(|y| f.get(x, y) != 0))
            .collect();
        (rows, columns)
    }

    #[test]
    fn every_size_stands_in_the_rows_it_says_from_the_row_it_is_given() {
        for (size, rows) in [
            (Size::Small, 6),
            (Size::Medium, 8),
            (Size::Large, 10),
            (Size::ExtraLarge, 13),
        ] {
            assert_eq!(size.rows(), rows);
            let mut f = Frame::new();
            size.draw(&mut f, 10, 20, "0123456789", 9, 0);
            let (lit, _) = extent(&f);
            assert_eq!(lit, (20..20 + rows).collect::<Vec<_>>(), "{size:?}");
            // Each figure alone fills those rows from the first to the last:
            // but for a figure of segments that has no top or no bottom one.
            for d in 0..10 {
                if size == Size::ExtraLarge && [1, 4, 7].contains(&d) {
                    continue;
                }
                let mut f = Frame::new();
                size.draw(&mut f, 10, 20, &d.to_string(), 9, 0);
                let (lit, _) = extent(&f);
                assert_eq!(
                    (lit.first(), lit.last()),
                    (Some(&20), Some(&(20 + rows - 1))),
                    "{size:?} {d}"
                );
            }
        }
    }

    #[test]
    fn a_time_is_as_wide_as_it_says_and_starts_at_its_column() {
        for size in SIZES {
            for time in ["5:12", "20:34", "125:07", "8:88"] {
                let mut f = Frame::new();
                size.draw(&mut f, 30, 20, time, 9, 0);
                let (_, lit) = extent(&f);
                let width = size.width(time);
                assert!(
                    *lit.first().unwrap() >= 30 && *lit.last().unwrap() < 30 + width,
                    "{size:?} {time}: {lit:?} in {width}"
                );
                // An eight reaches both edges of its figure.
                if time == "8:88" {
                    assert_eq!(
                        (*lit.first().unwrap(), *lit.last().unwrap()),
                        (30, 30 + width - 1),
                        "{size:?}"
                    );
                }
            }
        }
        assert_eq!(Size::Small.width("5:12"), 19);
        assert_eq!(Size::Medium.width("5:12"), 21);
        assert_eq!(Size::Large.width("5:12"), 27);
        assert_eq!(Size::ExtraLarge.width("5:12"), 26);
        assert_eq!(Size::Small.width(""), 0);
    }

    #[test]
    fn a_time_padded_to_the_length_of_another_is_as_wide_and_ends_where_it_would() {
        for size in SIZES {
            assert_eq!(size.width(" 9:59"), size.width("10:00"), "{size:?}");
            let mut padded = Frame::new();
            size.draw(&mut padded, 30, 20, " 9:59", 9, 0);
            let mut alone = Frame::new();
            let lead = size.width(" 9:59") - size.width("9:59");
            size.draw(&mut alone, 30 + lead, 20, "9:59", 9, 0);
            assert_eq!(padded, alone, "{size:?}");
        }
    }

    #[test]
    fn the_eight_row_figures_are_five_wide_and_the_colon_has_two_dots() {
        for (d, figure) in MEDIUM.iter().enumerate() {
            assert!(figure.iter().all(|row| row.len() == 5), "{d}");
        }
        let mut f = Frame::new();
        Size::Medium.draw(&mut f, 0, 0, ":", 9, 0);
        let lit: Vec<(u32, u32)> = (0..8)
            .flat_map(|y| (0..3).map(move |x| (x, y)))
            .filter(|&(x, y)| f.get(x, y) != 0)
            .collect();
        assert_eq!(lit, [(1, 2), (1, 5)]);
        // A one and a seven are told apart, and a six from an eight.
        assert_ne!(MEDIUM[1], MEDIUM[7]);
        assert_ne!(MEDIUM[6], MEDIUM[8]);
    }

    #[test]
    fn the_segment_figures_light_what_the_clocks_do_and_ghost_the_rest() {
        let mut f = Frame::new();
        Size::ExtraLarge.draw(&mut f, 0, 0, "1", 9, 2);
        // The two bars at the right are lit, two columns each.
        assert_eq!((f.get(4, 3), f.get(5, 3)), (9, 9));
        assert_eq!((f.get(4, 9), f.get(5, 9)), (9, 9));
        // The top, the middle, the bottom and the left are ghosts.
        assert_eq!(f.get(2, 0), 2);
        assert_eq!(f.get(2, 6), 2);
        assert_eq!(f.get(2, 12), 2);
        assert_eq!((f.get(0, 3), f.get(0, 9)), (2, 2));
        // The corners are no segment's.
        assert_eq!((f.get(0, 0), f.get(5, 12)), (0, 0));

        // The middle segment is one row, and the halves either side match.
        let mut eight = Frame::new();
        Size::ExtraLarge.draw(&mut eight, 0, 0, "8", 9, 0);
        assert!((1..5).all(|x| eight.get(x, 6) == 9));
        assert!((2..4).all(|x| eight.get(x, 5) == 0 && eight.get(x, 7) == 0));
        for row in 0..6 {
            for x in 0..6 {
                assert_eq!(eight.get(x, row), eight.get(x, 12 - row), "({x}, {row})");
            }
        }

        // A space is a figure with nothing lit: dark without a ghost.
        let mut space = Frame::new();
        Size::ExtraLarge.draw(&mut space, 0, 0, " ", 9, 0);
        assert_eq!(space, Frame::new());
        let mut ghost = Frame::new();
        Size::ExtraLarge.draw(&mut ghost, 0, 0, " ", 9, 2);
        assert_eq!(ghost.get(2, 0), 2);
        assert!((0..13).all(|y| (0..6).all(|x| ghost.get(x, y) != 9)));
    }

    #[test]
    fn the_sizes_are_named_as_the_settings_name_them() {
        assert_eq!(Size::parse("small"), Some(Size::Small));
        assert_eq!(Size::parse(" medium "), Some(Size::Medium));
        assert_eq!(Size::parse("large"), Some(Size::Large));
        assert_eq!(Size::parse("extra_large"), Some(Size::ExtraLarge));
        assert_eq!(Size::parse("huge"), None);
        assert_eq!(Size::parse("off"), None);
    }
}
