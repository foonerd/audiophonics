//! The level meters on the first page.
//!
//! Two meters, the left channel's and the right's, on a scale of sixty
//! decibels. The bright part of a meter is the level, the dimmer part
//! beyond it reaches to the peak, and a mark holds the highest peak for a
//! moment. The scale's marks are named in decibels under full scale.
//!
//! The meters stand upright in a strip at one side of the cover, or lie in
//! a strip over the track's line. Either strip is veiled as the bar at the
//! top is, so the meters read the same over any cover.

use crate::canvas::{Area, Canvas, Rgb, BLACK, WIDTH};
use crate::config::Place;
use crate::screens::{BAR_VEIL, QUIET};
use crate::text::Faces;
use audiophonics_meters::{place, Bar};

/// Where the scale's marks stand, in decibels under full scale, and which
/// of them are named.
const MARKS: [u8; 9] = [0, 3, 6, 10, 20, 30, 40, 50, 60];
const NAMED: [u8; 5] = [0, 10, 20, 40, 60];

/// A meter with nothing in it, its part up to the peak, its part up to the
/// level, and the mark that holds.
const TROUGH: Rgb = [28, 28, 28];
const PEAK: Rgb = [120, 116, 108];
const LEVEL: Rgb = [236, 230, 216];
const HOLD: Rgb = [255, 150, 70];

/// The size of the scale's numbers and of the channels' letters.
const TEXT: u32 = 9;
/// How thick a meter is, and its holding mark.
const THICK: i32 = 6;
const HOLD_THICK: i32 = 2;

/// Upright: a strip beside the cover, from under the bar at the top to the
/// progress line. Full scale is at its top row, the floor at its bottom.
const STRIP_WIDTH: u32 = 34;
/// One meter and its scale, at each side.
const SIDE_WIDTH: u32 = 27;
const STRIP_TOP: u32 = 18;
const STRIP_HEIGHT: u32 = 189;
const UP_TOP: i32 = 30;
const UP_BOTTOM: i32 = 200;
const UP_LETTERS: i32 = 27;

/// Lying: a strip over the track's line. The floor is at its left end,
/// full scale at its right.
const LYING: Area = Area::new(0, 178, WIDTH, 29);
const LY_LEFT: i32 = 16;
const LY_RIGHT: i32 = 312;
const LY_ROWS: [i32; 2] = [181, 199];
const LY_MARKS: i32 = 188;
const LY_NUMBERS: i32 = 197;
const LY_LETTERS: i32 = 4;

/// So many of `pixels` as a place along the scale takes.
fn along(pixels: i32, place: f32) -> i32 {
    (pixels as f32 * place.clamp(0.0, 1.0)).round() as i32
}

/// A rectangle by its corner and size, where it has any size.
fn fill(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32, colour: Rgb) {
    if x >= 0 && y >= 0 && w > 0 && h > 0 {
        canvas.fill_area(Area::new(x as u32, y as u32, w as u32, h as u32), colour);
    }
}

/// Draw the meters where they stand.
pub fn draw(canvas: &mut Canvas, place: Place, bars: &[Bar; 2], faces: &mut Faces) {
    let [left, right] = bars;
    match place {
        // Both in one strip, the left channel the first met from the left,
        // the scale on the cover's side of them.
        Place::Right => {
            let x0 = (WIDTH - STRIP_WIDTH) as i32;
            strip(canvas, x0, STRIP_WIDTH);
            scale(canvas, x0 + 13, x0 + 12, true, faces);
            column(canvas, x0 + 17, left, "L", faces);
            column(canvas, x0 + 25, right, "R", faces);
        }
        Place::Left => {
            strip(canvas, 0, STRIP_WIDTH);
            column(canvas, 3, left, "L", faces);
            column(canvas, 11, right, "R", faces);
            scale(canvas, 18, 22, false, faces);
        }
        // One in a strip of its own at each side, each with its scale on
        // the cover's side of it.
        Place::Sides => {
            strip(canvas, 0, SIDE_WIDTH);
            column(canvas, 3, left, "L", faces);
            scale(canvas, 10, 14, false, faces);
            let x0 = (WIDTH - SIDE_WIDTH) as i32;
            strip(canvas, x0, SIDE_WIDTH);
            scale(canvas, x0 + 14, x0 + 13, true, faces);
            column(canvas, x0 + 18, right, "R", faces);
        }
        Place::Over => lying(canvas, bars, faces),
    }
}

/// The row a level stands at on an upright meter: the top row of a meter
/// filled that far.
fn up_row(db_under: u8) -> i32 {
    let length = along(UP_BOTTOM - UP_TOP + 1, place(-f32::from(db_under)));
    (UP_BOTTOM - length + 1).min(UP_BOTTOM)
}

/// The veil an upright strip stands on, `width` wide from column `x0`.
fn strip(canvas: &mut Canvas, x0: i32, width: u32) {
    canvas.veil(
        Area::new(x0 as u32, STRIP_TOP, width, STRIP_HEIGHT),
        BLACK,
        BAR_VEIL,
    );
}

/// One upright meter with its channel's letter over it, its left column
/// at `x`.
fn column(canvas: &mut Canvas, x: i32, bar: &Bar, letter: &str, faces: &mut Faces) {
    let span = UP_BOTTOM - UP_TOP + 1;
    let centred = x + (THICK - faces.width(letter, TEXT)) / 2;
    faces.draw(canvas, letter, centred, UP_LETTERS, TEXT, QUIET);
    fill(canvas, x, UP_TOP, THICK, span, TROUGH);
    for (part, colour) in [(bar.peak, PEAK), (bar.level, LEVEL)] {
        let length = along(span, part);
        fill(canvas, x, UP_BOTTOM - length + 1, THICK, length, colour);
    }
    let held = along(span, bar.hold);
    if held > 0 {
        let top = (UP_BOTTOM - held + 1).min(UP_BOTTOM - HOLD_THICK + 1);
        fill(canvas, x, top, THICK, HOLD_THICK, HOLD);
    }
}

/// An upright scale: its marks from column `marks`, and the numbers of
/// those that are named, which begin at column `numbers` or, `ending`
/// there, end before it.
fn scale(canvas: &mut Canvas, marks: i32, numbers: i32, ending: bool, faces: &mut Faces) {
    for db in MARKS {
        let row = up_row(db);
        fill(canvas, marks, row, 3, 1, QUIET);
        if NAMED.contains(&db) {
            let name = db.to_string();
            let x = if ending {
                numbers - faces.width(&name, TEXT)
            } else {
                numbers
            };
            faces.draw(canvas, &name, x, row + 3, TEXT, QUIET);
        }
    }
}

/// The column a level stands at on a lying meter: the last column of a
/// meter filled that far.
fn ly_column(db_under: u8) -> i32 {
    let length = along(LY_RIGHT - LY_LEFT + 1, place(-f32::from(db_under)));
    (LY_LEFT + length - 1).max(LY_LEFT)
}

fn lying(canvas: &mut Canvas, bars: &[Bar; 2], faces: &mut Faces) {
    canvas.veil(LYING, BLACK, BAR_VEIL);
    let span = LY_RIGHT - LY_LEFT + 1;
    for ((bar, y), letter) in bars.iter().zip(LY_ROWS).zip(["L", "R"]) {
        faces.draw(canvas, letter, LY_LETTERS, y + THICK + 1, TEXT, QUIET);
        fill(canvas, LY_LEFT, y, span, THICK, TROUGH);
        for (part, colour) in [(bar.peak, PEAK), (bar.level, LEVEL)] {
            fill(canvas, LY_LEFT, y, along(span, part), THICK, colour);
        }
        let held = along(span, bar.hold);
        if held > 0 {
            let left = (LY_LEFT + held - HOLD_THICK).max(LY_LEFT);
            fill(canvas, left, y, HOLD_THICK, THICK, HOLD);
        }
    }
    for db in MARKS {
        let column = ly_column(db);
        fill(canvas, column, LY_MARKS, 1, 2, QUIET);
        if NAMED.contains(&db) {
            let name = db.to_string();
            let width = faces.width(&name, TEXT);
            let x = (column - width / 2).min(WIDTH as i32 - 1 - width);
            faces.draw(canvas, &name, x, LY_NUMBERS, TEXT, QUIET);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::HEIGHT;
    use std::path::PathBuf;

    fn assets() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../plugins/audiophonics_raspdacminilcd/assets")
    }

    fn bar(level: f32, peak: f32, hold: f32) -> Bar {
        Bar {
            level: place(level),
            peak: place(peak),
            hold: place(hold),
        }
    }

    /// A page of one colour that is none of the meters', with them on it.
    const GROUND: Rgb = [0, 90, 200];
    fn drawn(place: Place, bars: [Bar; 2]) -> Canvas {
        let mut faces = Faces::open(&assets());
        let mut canvas = Canvas::new();
        canvas.fill(GROUND);
        draw(&mut canvas, place, &bars, &mut faces);
        canvas
    }

    fn count(
        canvas: &Canvas,
        colour: Rgb,
        x: std::ops::Range<i32>,
        y: std::ops::Range<i32>,
    ) -> usize {
        y.flat_map(|y| x.clone().map(move |x| (x, y)))
            .filter(|&(x, y)| canvas.get(x, y) == colour)
            .count()
    }

    #[test]
    fn the_meters_keep_to_their_strip() {
        let loud = [bar(-3.0, -1.0, 0.0), bar(-6.0, -2.0, -1.0)];
        for (place, strip) in [
            (Place::Right, (286..320, 18..207)),
            (Place::Left, (0..34, 18..207)),
            (Place::Sides, (27..293, 0..0)),
            (Place::Over, (0..320, 178..207)),
        ] {
            let canvas = drawn(place, loud);
            for y in 0..HEIGHT as i32 {
                for x in 0..WIDTH as i32 {
                    // At both sides the strips are what is outside the
                    // middle; elsewhere the strip is what is named.
                    let inside = if place == Place::Sides {
                        !strip.0.contains(&x) && (18..207).contains(&y)
                    } else {
                        strip.0.contains(&x) && strip.1.contains(&y)
                    };
                    if !inside {
                        assert_eq!(canvas.get(x, y), GROUND, "{place:?} at {x},{y}");
                    }
                }
            }
        }
    }

    #[test]
    fn an_upright_meter_is_as_tall_as_its_level() {
        // Half way down the scale is half the meter: 171 rows of it, six
        // across.
        let canvas = drawn(
            Place::Right,
            [bar(-30.0, -30.0, -30.0), bar(-60.0, -60.0, -60.0)],
        );
        let left = 286 + 17..286 + 23;
        let right = 286 + 25..286 + 31;
        // The mark that holds lies over the meter's top two rows.
        assert_eq!(count(&canvas, LEVEL, left.clone(), 18..207), (86 - 2) * 6);
        assert_eq!(count(&canvas, HOLD, left.clone(), 18..207), 2 * 6);
        assert_eq!(count(&canvas, TROUGH, left, 18..207), (171 - 86) * 6);
        // Silence: a trough and nothing in it.
        assert_eq!(count(&canvas, TROUGH, right.clone(), 18..207), 171 * 6);
        assert_eq!(count(&canvas, LEVEL, right.clone(), 18..207), 0);
        assert_eq!(count(&canvas, HOLD, right, 18..207), 0);

        // Full scale fills it to its top row.
        let full = drawn(Place::Left, [bar(0.0, 0.0, 0.0), bar(0.0, 0.0, 0.0)]);
        assert_eq!(full.get(3, UP_TOP), HOLD);
        assert_eq!(full.get(3, UP_TOP + 2), LEVEL);
        assert_eq!(full.get(3, UP_BOTTOM), LEVEL);
        assert_ne!(full.get(3, UP_TOP - 1), LEVEL);
    }

    #[test]
    fn the_peak_shows_beyond_the_level_and_the_mark_beyond_both() {
        let canvas = drawn(
            Place::Left,
            [bar(-40.0, -20.0, -10.0), bar(-60.0, -60.0, -60.0)],
        );
        let x = 4;
        assert_eq!(canvas.get(x, up_row(40) + 1), LEVEL);
        assert_eq!(canvas.get(x, up_row(40) - 1), PEAK);
        assert_eq!(canvas.get(x, up_row(20)), PEAK);
        assert_eq!(canvas.get(x, up_row(20) - 1), TROUGH);
        assert_eq!(canvas.get(x, up_row(10)), HOLD);
        assert_eq!(canvas.get(x, up_row(10) - 1), TROUGH);
    }

    #[test]
    fn a_level_stands_at_the_scales_mark_for_it() {
        // Upright: the top row of a meter at -20 dB is the row of the
        // scale's mark for 20.
        let canvas = drawn(
            Place::Left,
            [bar(-20.0, -20.0, -60.0), bar(-60.0, -60.0, -60.0)],
        );
        let row = up_row(20);
        assert_eq!(canvas.get(3, row), LEVEL);
        assert_eq!(canvas.get(3, row - 1), TROUGH);
        assert_eq!(canvas.get(18, row), QUIET, "the mark");
        // Lying: its last column is the column of the mark.
        let canvas = drawn(
            Place::Over,
            [bar(-20.0, -20.0, -60.0), bar(-60.0, -60.0, -60.0)],
        );
        let column = ly_column(20);
        assert_eq!(canvas.get(column, LY_ROWS[0]), LEVEL);
        assert_eq!(canvas.get(column + 1, LY_ROWS[0]), TROUGH);
        assert_eq!(canvas.get(column, LY_MARKS), QUIET, "the mark");
        // The ends of the scale are the ends of the meter.
        assert_eq!((up_row(0), up_row(60)), (UP_TOP, UP_BOTTOM));
        assert_eq!((ly_column(60), ly_column(0)), (LY_LEFT, LY_RIGHT));
    }

    #[test]
    fn at_both_sides_each_channel_has_its_own_side() {
        // The left channel at half the scale, the right silent.
        let canvas = drawn(
            Place::Sides,
            [bar(-30.0, -30.0, -60.0), bar(-60.0, -60.0, -60.0)],
        );
        assert_eq!(count(&canvas, LEVEL, 0..27, 18..207), 86 * 6);
        assert_eq!(count(&canvas, LEVEL, 293..320, 18..207), 0);
        assert_eq!(count(&canvas, TROUGH, 293..320, 18..207), 171 * 6);
        // And the other way about.
        let canvas = drawn(
            Place::Sides,
            [bar(-60.0, -60.0, -60.0), bar(-30.0, -30.0, -60.0)],
        );
        assert_eq!(count(&canvas, LEVEL, 0..27, 18..207), 0);
        assert_eq!(count(&canvas, LEVEL, 293..320, 18..207), 86 * 6);
        // Each side has the scale's marks, at the same rows.
        let row = up_row(20);
        assert_eq!(canvas.get(10, row), QUIET);
        assert_eq!(canvas.get(293 + 14, row), QUIET);
    }

    #[test]
    fn a_lying_meter_is_as_long_as_its_level() {
        let canvas = drawn(Place::Over, [bar(-30.0, -30.0, -60.0), bar(0.0, 0.0, 0.0)]);
        let span = (LY_RIGHT - LY_LEFT + 1) as usize;
        assert_eq!(span, 297);
        let upper = LY_ROWS[0]..LY_ROWS[0] + 6;
        let lower = LY_ROWS[1]..LY_ROWS[1] + 6;
        assert_eq!(count(&canvas, LEVEL, 0..320, upper.clone()), 149 * 6);
        assert_eq!(count(&canvas, TROUGH, 0..320, upper), (297 - 149) * 6);
        assert_eq!(count(&canvas, LEVEL, 0..320, lower.clone()), (297 - 2) * 6);
        assert_eq!(count(&canvas, HOLD, 0..320, lower), 2 * 6);
    }
}
