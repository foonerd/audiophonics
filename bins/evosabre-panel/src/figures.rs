//! The figures of the track's time, at four sizes.
//!
//! The smallest two keep the track's three lines above them: the face the
//! time has always had, six rows tall, and one drawn here, eight rows tall,
//! which is as tall as a time can be under an album's line. The larger two
//! take the album's rows: a face of the panel's own, ten rows tall, and
//! figures of thirteen rows, in one of three faces: seven segments, drawn
//! as the resting clock's are; a light face drawn here, with strokes of one
//! pixel; and a bold one of the panel's own, wider and wider apart.
//!
//! A time is placed by the top row of its figures and by its first lit
//! column, whatever the face, and a space in it is as wide as a figure: a
//! time padded to the length of another is as wide as that other.

use crate::frame::Frame;
use crate::segments;
use embedded_graphics::mono_font::iso_8859_1::{FONT_10X20, FONT_5X8, FONT_7X14};
use embedded_graphics::mono_font::{MonoFont, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::text::{Baseline, Text};

/// How large the time is drawn, and at its largest in which face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Small,
    Medium,
    Large,
    ExtraLarge(Face),
}

/// The face of the largest figures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    /// Seven segments, as the clock's.
    Segments,
    /// Strokes of one pixel.
    Light,
    /// Heavier strokes, wider figures, wider apart.
    Bold,
}

#[cfg(test)]
pub const FACES: [Face; 3] = [Face::Segments, Face::Light, Face::Bold];

#[cfg(test)]
pub const SIZES: [Size; 6] = [
    Size::Small,
    Size::Medium,
    Size::Large,
    Size::ExtraLarge(Face::Segments),
    Size::ExtraLarge(Face::Light),
    Size::ExtraLarge(Face::Bold),
];

/// A face of the panel's own, and where its figures stand in its cell: the
/// rows above their top, the columns before their first, and the columns
/// they are wide. What is left of the cell's width stands between two.
struct Type {
    font: &'static MonoFont<'static>,
    above: u32,
    before: u32,
    wide: u32,
}

const SMALL: Type = Type {
    font: &FONT_5X8,
    above: 1,
    before: 0,
    wide: 4,
};
const LARGE: Type = Type {
    font: &FONT_7X14,
    above: 2,
    before: 0,
    wide: 6,
};
const BOLD: Type = Type {
    font: &FONT_10X20,
    above: 3,
    before: 1,
    wide: 8,
};

impl Type {
    fn between(&self) -> u32 {
        self.font.character_size.width - self.wide
    }
}

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
/// segment is one row and the two halves match. The upright segments run
/// to the top row and to the bottom one, where the clock's stop short: at
/// this size a one or a four would otherwise be visibly shorter than the
/// figures beside it.
const SEGMENT_ROWS: u32 = 13;
const SEGMENT_W: u32 = 6;
const SEGMENT_T: u32 = 2;
const SEGMENT_GAP: u32 = 2;
const SEGMENT_MIDDLE: u32 = 6;
/// The top rows of the colon's two dots, each two pixels square: the
/// light figures have the same colon.
const SEGMENT_COLON: [u32; 2] = [3, 8];

/// The light thirteen-row figures: seven columns each, strokes of one
/// pixel, two columns between two, and the colon of the segments.
const LIGHT_W: u32 = 7;
const LIGHT_GAP: u32 = 2;
const LIGHT: [[&str; 13]; 10] = [
    [
        "..XXX..", ".X...X.", "X.....X", "X.....X", "X.....X", "X.....X", "X.....X", "X.....X",
        "X.....X", "X.....X", "X.....X", ".X...X.", "..XXX..",
    ],
    [
        "...X...", "..XX...", ".X.X...", "...X...", "...X...", "...X...", "...X...", "...X...",
        "...X...", "...X...", "...X...", "...X...", ".XXXXX.",
    ],
    [
        "..XXX..", ".X...X.", "X.....X", "......X", "......X", ".....X.", "....X..", "...X...",
        "..X....", ".X.....", "X......", "X......", "XXXXXXX",
    ],
    [
        "..XXX..", ".X...X.", "X.....X", "......X", "......X", ".....X.", "..XXX..", ".....X.",
        "......X", "......X", "X.....X", ".X...X.", "..XXX..",
    ],
    [
        ".....X.", "....XX.", "...X.X.", "...X.X.", "..X..X.", "..X..X.", ".X...X.", "X....X.",
        "XXXXXXX", ".....X.", ".....X.", ".....X.", ".....X.",
    ],
    [
        "XXXXXXX", "X......", "X......", "X......", "X.XXX..", "XX...X.", "......X", "......X",
        "......X", "......X", "X.....X", ".X...X.", "..XXX..",
    ],
    [
        "..XXX..", ".X...X.", "X......", "X......", "X......", "X.XXX..", "XX...X.", "X.....X",
        "X.....X", "X.....X", "X.....X", ".X...X.", "..XXX..",
    ],
    [
        "XXXXXXX", "......X", "......X", ".....X.", ".....X.", "....X..", "....X..", "...X...",
        "...X...", "..X....", "..X....", "..X....", "..X....",
    ],
    [
        "..XXX..", ".X...X.", "X.....X", "X.....X", "X.....X", ".X...X.", "..XXX..", ".X...X.",
        "X.....X", "X.....X", "X.....X", ".X...X.", "..XXX..",
    ],
    [
        "..XXX..", ".X...X.", "X.....X", "X.....X", "X.....X", "X.....X", ".X...XX", "..XXX.X",
        "......X", "......X", "......X", ".X...X.", "..XXX..",
    ],
];

impl Face {
    /// The face the settings name `s`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "segments" => Some(Self::Segments),
            "light" => Some(Self::Light),
            "bold" => Some(Self::Bold),
            _ => None,
        }
    }
}

impl Size {
    /// The size the settings name `s`: the largest in segments, until
    /// [`Size::in_face`] says otherwise.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "large" => Some(Self::Large),
            "extra_large" => Some(Self::ExtraLarge(Face::Segments)),
            _ => None,
        }
    }

    /// This size in `face`, where it has faces to choose from: the largest
    /// has, the others are as they are.
    pub fn in_face(self, face: Face) -> Self {
        match self {
            Self::ExtraLarge(_) => Self::ExtraLarge(face),
            other => other,
        }
    }

    /// The rows its figures stand in.
    pub fn rows(self) -> u32 {
        match self {
            Self::Small => 6,
            Self::Medium => MEDIUM_ROWS,
            Self::Large => 10,
            Self::ExtraLarge(_) => SEGMENT_ROWS,
        }
    }

    /// The face of the panel's own it is drawn in, if it is.
    fn typed(self) -> Option<&'static Type> {
        match self {
            Self::Small => Some(&SMALL),
            Self::Large => Some(&LARGE),
            Self::ExtraLarge(Face::Bold) => Some(&BOLD),
            _ => None,
        }
    }

    /// The columns between two of its characters.
    fn between(self) -> u32 {
        match (self, self.typed()) {
            (_, Some(face)) => face.between(),
            (Self::ExtraLarge(Face::Light), _) => LIGHT_GAP,
            (Self::ExtraLarge(_), _) => SEGMENT_GAP,
            _ => 1,
        }
    }

    /// The columns one character is drawn in.
    fn drawn(self, c: char) -> u32 {
        match (self, self.typed(), c) {
            (_, Some(face), _) => face.wide,
            (Self::Medium, _, ':') => MEDIUM_COLON_W,
            (Self::Medium, _, _) => MEDIUM_W,
            (Self::ExtraLarge(_), _, ':') => SEGMENT_T,
            (Self::ExtraLarge(Face::Light), _, _) => LIGHT_W,
            _ => SEGMENT_W,
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
    /// Where the figures are of segments, those that are not lit are drawn
    /// at `unlit`, as the clock's are; no other figures have any.
    pub fn draw(self, f: &mut Frame, x: u32, top: u32, time: &str, luma: u8, unlit: u8) {
        if let Some(face) = self.typed() {
            return typed(f, face, x, top, time, luma);
        }
        let mut cx = x;
        for c in time.chars() {
            match (self, c.to_digit(10)) {
                (Self::Medium, Some(d)) => figure(f, cx, top, &MEDIUM[d as usize], luma),
                (Self::Medium, None) if c == ':' => {
                    for row in MEDIUM_COLON {
                        f.set(cx + 1, top + row, luma);
                    }
                }
                (Self::ExtraLarge(Face::Light), Some(d)) => {
                    figure(f, cx, top, &LIGHT[d as usize], luma)
                }
                (Self::ExtraLarge(Face::Segments), Some(d)) => {
                    seven(f, cx, top, Some(d as u8), luma, unlit)
                }
                (Self::ExtraLarge(_), None) if c == ':' => {
                    for row in SEGMENT_COLON {
                        block(f, cx, top + row, SEGMENT_T, SEGMENT_T, luma);
                    }
                }
                // A space: a figure of segments with none lit.
                (Self::ExtraLarge(Face::Segments), None) => seven(f, cx, top, None, luma, unlit),
                _ => {}
            }
            cx += self.drawn(c) + self.between();
        }
    }
}

/// A time in one of the panel's own faces: its first figure's first column
/// on `x`.
fn typed(f: &mut Frame, face: &Type, x: u32, top: u32, time: &str, luma: u8) {
    Text::with_baseline(
        time,
        Point::new(
            x as i32 - face.before as i32,
            top as i32 - face.above as i32,
        ),
        MonoTextStyle::new(face.font, Gray4::new(luma)),
        Baseline::Top,
    )
    .draw(f)
    .ok();
}

/// One figure of those drawn here, a row of it to each line.
fn figure(f: &mut Frame, x: u32, top: u32, rows: &[&str], luma: u8) {
    for (row, line) in rows.iter().enumerate() {
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
    // its width and its height. An upright one shares its end with the
    // segment across it.
    let boxes = [
        (1, 0, w - 2, t),
        (w - t, 0, t, mid),
        (w - t, mid + 1, t, h - mid - 1),
        (1, h - t, w - 2, t),
        (0, mid + 1, t, h - mid - 1),
        (0, 0, t, mid),
        (1, mid, w - 2, 1),
    ];
    // The unlit ones first: where two segments share pixels, the lit one
    // has them.
    for lit_ones in [false, true] {
        for (bit, (bx, by, bw, bh)) in boxes.into_iter().enumerate() {
            let on = mask & (1 << bit) != 0;
            if on == lit_ones && (on || unlit > 0) {
                block(f, x + bx, y + by, bw, bh, if on { lit } else { unlit });
            }
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
            (Size::ExtraLarge(Face::Segments), 13),
            (Size::ExtraLarge(Face::Light), 13),
            (Size::ExtraLarge(Face::Bold), 13),
        ] {
            assert_eq!(size.rows(), rows);
            let mut f = Frame::new();
            size.draw(&mut f, 10, 20, "0123456789", 9, 0);
            let (lit, _) = extent(&f);
            assert_eq!(lit, (20..20 + rows).collect::<Vec<_>>(), "{size:?}");
            // Each figure alone fills those rows from the first to the last:
            // a one is as tall as an eight, in segments too.
            for d in 0..10 {
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
        assert_eq!(Size::ExtraLarge(Face::Segments).width("5:12"), 26);
        assert_eq!(Size::ExtraLarge(Face::Light).width("5:12"), 29);
        assert_eq!(Size::ExtraLarge(Face::Bold).width("5:12"), 38);
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
        Size::ExtraLarge(Face::Segments).draw(&mut f, 0, 0, "1", 9, 2);
        // The two bars at the right are lit, two columns each.
        assert_eq!((f.get(4, 3), f.get(5, 3)), (9, 9));
        assert_eq!((f.get(4, 9), f.get(5, 9)), (9, 9));
        // They run from the top row to the bottom one, with the one row
        // of the waist between them.
        assert_eq!((f.get(4, 0), f.get(5, 0)), (9, 9));
        assert_eq!((f.get(4, 12), f.get(5, 12)), (9, 9));
        assert_eq!(f.get(5, 6), 0);
        // The top, the middle, the bottom and the left are ghosts.
        assert_eq!(f.get(2, 0), 2);
        assert_eq!(f.get(2, 6), 2);
        assert_eq!(f.get(2, 12), 2);
        assert_eq!(
            (f.get(0, 0), f.get(0, 3), f.get(0, 9), f.get(0, 12)),
            (2, 2, 2, 2)
        );

        // Where a lit segment and an unlit one share pixels, they are lit:
        // a seven's top runs to its second column though its left is dark.
        let mut seven = Frame::new();
        Size::ExtraLarge(Face::Segments).draw(&mut seven, 0, 0, "7", 9, 2);
        assert_eq!(
            (seven.get(0, 0), seven.get(1, 0), seven.get(1, 1)),
            (2, 9, 9)
        );
        assert_eq!(seven.get(1, 2), 2, "under the top, the left is a ghost");
        // And a four is its left half, its waist and its right, top to bottom.
        let mut four = Frame::new();
        Size::ExtraLarge(Face::Segments).draw(&mut four, 0, 0, "4", 9, 0);
        assert_eq!((four.get(0, 0), four.get(0, 5), four.get(0, 7)), (9, 9, 0));
        assert_eq!((four.get(5, 0), four.get(5, 12), four.get(2, 6)), (9, 9, 9));
        assert_eq!((four.get(2, 0), four.get(2, 12)), (0, 0));

        // The middle segment is one row, and the halves either side match.
        let mut eight = Frame::new();
        Size::ExtraLarge(Face::Segments).draw(&mut eight, 0, 0, "8", 9, 0);
        assert!((1..5).all(|x| eight.get(x, 6) == 9));
        assert!((2..4).all(|x| eight.get(x, 5) == 0 && eight.get(x, 7) == 0));
        for row in 0..6 {
            for x in 0..6 {
                assert_eq!(eight.get(x, row), eight.get(x, 12 - row), "({x}, {row})");
            }
        }

        // A space is a figure with nothing lit: dark without a ghost.
        let mut space = Frame::new();
        Size::ExtraLarge(Face::Segments).draw(&mut space, 0, 0, " ", 9, 0);
        assert_eq!(space, Frame::new());
        let mut ghost = Frame::new();
        Size::ExtraLarge(Face::Segments).draw(&mut ghost, 0, 0, " ", 9, 2);
        assert_eq!(ghost.get(2, 0), 2);
        assert!((0..13).all(|y| (0..6).all(|x| ghost.get(x, y) != 9)));
    }

    #[test]
    fn the_light_figures_are_seven_wide_with_strokes_of_one_pixel() {
        for (d, figure) in LIGHT.iter().enumerate() {
            assert!(figure.iter().all(|row| row.len() == 7), "{d}");
            // A stroke thicker than one pixel would light a square of four
            // somewhere.
            let lit = |x: usize, y: usize| figure[y].as_bytes()[x] == b'X';
            for y in 0..12 {
                for x in 0..6 {
                    assert!(
                        !(lit(x, y) && lit(x + 1, y) && lit(x, y + 1) && lit(x + 1, y + 1)),
                        "{d} at ({x}, {y})"
                    );
                }
            }
        }
        // Each is told from every other.
        for (a, one) in LIGHT.iter().enumerate() {
            for (b, other) in LIGHT.iter().enumerate().skip(a + 1) {
                assert_ne!(one, other, "{a} and {b}");
            }
        }
        // It has the colon of the segments, and no ghosts.
        let mut colon = Frame::new();
        Size::ExtraLarge(Face::Light).draw(&mut colon, 0, 0, ":", 9, 2);
        let mut same = Frame::new();
        Size::ExtraLarge(Face::Segments).draw(&mut same, 0, 0, ":", 9, 2);
        assert_eq!(colon, same);
        for face in [Face::Light, Face::Bold] {
            let mut plain = Frame::new();
            Size::ExtraLarge(face).draw(&mut plain, 0, 0, " 1:07", 9, 0);
            let mut ghosted = Frame::new();
            Size::ExtraLarge(face).draw(&mut ghosted, 0, 0, " 1:07", 9, 2);
            assert_eq!(plain, ghosted, "{face:?}");
        }
    }

    #[test]
    fn the_three_faces_of_the_largest_differ_in_weight_and_in_width() {
        let lit = |face: Face| {
            let mut f = Frame::new();
            Size::ExtraLarge(face).draw(&mut f, 0, 0, "0123456789", 9, 0);
            (0..13)
                .flat_map(|y| (0..110).map(move |x| (x, y)))
                .filter(|&(x, y)| f.get(x, y) != 0)
                .count()
        };
        // The light face lights the fewest pixels, the segments the most
        // for their width.
        assert!(lit(Face::Light) < lit(Face::Bold));
        assert!(lit(Face::Light) < lit(Face::Segments));
        let width = |face: Face| Size::ExtraLarge(face).width("3:48");
        assert_eq!(
            (width(Face::Segments), width(Face::Light), width(Face::Bold)),
            (26, 29, 38)
        );
    }

    #[test]
    fn the_sizes_are_named_as_the_settings_name_them() {
        assert_eq!(Size::parse("small"), Some(Size::Small));
        assert_eq!(Size::parse(" medium "), Some(Size::Medium));
        assert_eq!(Size::parse("large"), Some(Size::Large));
        assert_eq!(
            Size::parse("extra_large"),
            Some(Size::ExtraLarge(Face::Segments))
        );
        // The largest has faces to choose from; the others are as they are.
        for face in FACES {
            assert_eq!(
                Size::parse("extra_large").unwrap().in_face(face),
                Size::ExtraLarge(face)
            );
            assert_eq!(Size::Large.in_face(face), Size::Large);
            assert_eq!(Size::Small.in_face(face), Size::Small);
        }
        assert_eq!(Face::parse("segments"), Some(Face::Segments));
        assert_eq!(Face::parse(" light "), Some(Face::Light));
        assert_eq!(Face::parse("bold"), Some(Face::Bold));
        assert_eq!(Face::parse("plain"), None);
        assert_eq!(Size::parse("huge"), None);
        assert_eq!(Size::parse("off"), None);
    }
}
