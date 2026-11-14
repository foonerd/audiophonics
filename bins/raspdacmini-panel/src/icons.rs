//! The marks of the top bar: what the player is doing, repeat, shuffle and
//! the speaker. Each is a small drawing of whole pixels, ten rows high,
//! which at this size is sharper than any outline.

use crate::canvas::{Canvas, Rgb};

pub struct Icon {
    rows: &'static [&'static str],
}

impl Icon {
    #[cfg(test)]
    pub fn width(&self) -> u32 {
        self.rows.iter().map(|r| r.len()).max().unwrap_or(0) as u32
    }

    /// Draw the icon with its left top corner at (`x`, `y`), in `colour`
    /// laid on as far as `cover` says: a mark that is off is drawn faint.
    pub fn draw(&self, canvas: &mut Canvas, x: i32, y: i32, colour: Rgb, cover: u8) {
        for (row, line) in self.rows.iter().enumerate() {
            for (col, mark) in line.bytes().enumerate() {
                if mark == b'X' {
                    canvas.blend(x + col as i32, y + row as i32, colour, cover);
                }
            }
        }
    }
}

pub const PLAY: Icon = Icon {
    rows: &[
        "XX........",
        "XXXX......",
        "XXXXXX....",
        "XXXXXXXX..",
        "XXXXXXXXXX",
        "XXXXXXXXXX",
        "XXXXXXXX..",
        "XXXXXX....",
        "XXXX......",
        "XX........",
    ],
};

pub const PAUSE: Icon = Icon {
    rows: &[
        "XXX....XXX",
        "XXX....XXX",
        "XXX....XXX",
        "XXX....XXX",
        "XXX....XXX",
        "XXX....XXX",
        "XXX....XXX",
        "XXX....XXX",
        "XXX....XXX",
        "XXX....XXX",
    ],
};

pub const STOP: Icon = Icon {
    rows: &[
        "XXXXXXXXXX",
        "XXXXXXXXXX",
        "XXXXXXXXXX",
        "XXXXXXXXXX",
        "XXXXXXXXXX",
        "XXXXXXXXXX",
        "XXXXXXXXXX",
        "XXXXXXXXXX",
        "XXXXXXXXXX",
        "XXXXXXXXXX",
    ],
};

/// Two arrows chasing each other round: the queue plays again.
pub const REPEAT: Icon = Icon {
    rows: &[
        "....X........",
        "...XX........",
        "..XXXXXXXXX..",
        "...XX......X.",
        "....X......X.",
        ".X......X....",
        ".X......XX...",
        "..XXXXXXXXX..",
        "........XX...",
        "........X....",
    ],
};

/// The same with a 1 between the arrows: the track plays again.
pub const REPEAT_ONE: Icon = Icon {
    rows: &[
        "....X........",
        "...XX........",
        "..XXXXXXXXX..",
        "...XX.X....X.",
        "....XXX....X.",
        ".X....X.X....",
        ".X....X.XX...",
        "..XXXXXXXXX..",
        "........XX...",
        "........X....",
    ],
};

/// Two lines that cross and end in arrows: the queue in no order.
pub const SHUFFLE: Icon = Icon {
    rows: &[
        ".........X...",
        "XX.....XXXX..",
        "..X...X..X...",
        "...X.X.......",
        "....X........",
        "....X........",
        "...X.X.......",
        "..X...X..X...",
        "XX.....XXXX..",
        ".........X...",
    ],
};

const SPEAKER: Icon = Icon {
    rows: &[
        "....X", "...XX", "XXXXX", "XXXXX", "XXXXX", "XXXXX", "XXXXX", "XXXXX", "...XX", "....X",
    ],
};

/// The speaker's waves, nearest first: where each has a pixel, right of
/// the speaker's left edge and below its top.
const WAVES: [&[(i32, i32)]; 3] = [
    &[(7, 3), (8, 4), (8, 5), (7, 6)],
    &[
        (9, 1),
        (10, 2),
        (11, 3),
        (11, 4),
        (11, 5),
        (11, 6),
        (10, 7),
        (9, 8),
    ],
    &[
        (12, 0),
        (13, 1),
        (14, 2),
        (14, 3),
        (14, 4),
        (14, 5),
        (14, 6),
        (14, 7),
        (13, 8),
        (12, 9),
    ],
];

/// The cross beside a speaker that gives no sound.
const CROSS: &[(i32, i32)] = &[
    (8, 2),
    (9, 3),
    (10, 4),
    (11, 5),
    (12, 6),
    (13, 7),
    (13, 2),
    (12, 3),
    (11, 4),
    (10, 5),
    (9, 6),
    (8, 7),
];

/// How wide the speaker is with all its waves.
#[cfg(test)]
pub const SPEAKER_WIDTH: u32 = 15;

/// How many waves a volume has: none at nought, three from two thirds up.
pub fn waves(volume: u8) -> usize {
    match volume {
        0 => 0,
        1..=33 => 1,
        34..=66 => 2,
        _ => 3,
    }
}

/// The speaker with as many waves as the volume has, or crossed where the
/// volume is nought or unknown.
pub fn speaker(canvas: &mut Canvas, x: i32, y: i32, volume: Option<u8>, colour: Rgb) {
    SPEAKER.draw(canvas, x, y, colour, 255);
    let marks: Vec<(i32, i32)> = match volume {
        Some(v) if v > 0 => WAVES[..waves(v)]
            .iter()
            .flat_map(|w| w.iter().copied())
            .collect(),
        _ => CROSS.to_vec(),
    };
    for (dx, dy) in marks {
        canvas.set(x + dx, y + dy, colour);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{BLACK, WHITE};

    fn lit(canvas: &Canvas) -> usize {
        let mut n = 0;
        for y in 0..240 {
            for x in 0..320 {
                if canvas.get(x, y) != BLACK {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn every_icon_is_ten_rows_of_one_width() {
        for icon in [PLAY, PAUSE, STOP, REPEAT, REPEAT_ONE, SHUFFLE, SPEAKER] {
            assert_eq!(icon.rows.len(), 10);
            assert!(icon.rows.iter().all(|r| r.len() as u32 == icon.width()));
            assert!(icon
                .rows
                .iter()
                .all(|r| r.bytes().all(|b| b == b'X' || b == b'.')));
        }
        assert_eq!(REPEAT.width(), REPEAT_ONE.width());
    }

    #[test]
    fn an_icon_is_drawn_where_asked_as_strongly_as_asked() {
        let mut canvas = Canvas::new();
        PLAY.draw(&mut canvas, 4, 4, WHITE, 255);
        assert_eq!(canvas.get(4, 4), WHITE);
        assert_eq!(canvas.get(13, 8), WHITE, "the point of the triangle");
        assert_eq!(canvas.get(13, 4), BLACK);
        let mut faint = Canvas::new();
        SHUFFLE.draw(&mut faint, 0, 0, WHITE, 77);
        assert_eq!(faint.get(0, 1), [77, 77, 77]);
    }

    #[test]
    fn the_speaker_has_a_wave_for_each_third_of_the_volume_and_a_cross_at_nought() {
        assert_eq!(
            [
                waves(0),
                waves(1),
                waves(33),
                waves(34),
                waves(66),
                waves(67),
                waves(100)
            ],
            [0, 1, 1, 2, 2, 3, 3]
        );
        let count = |volume| {
            let mut canvas = Canvas::new();
            speaker(&mut canvas, 0, 0, volume, WHITE);
            lit(&canvas)
        };
        let body = 36;
        assert_eq!(count(Some(20)), body + 4);
        assert_eq!(count(Some(50)), body + 4 + 8);
        assert_eq!(count(Some(100)), body + 4 + 8 + 10);
        assert_eq!(count(Some(0)), body + 12, "crossed");
        assert_eq!(
            count(None),
            body + 12,
            "crossed where the volume is not known"
        );
        // Nothing is drawn beyond the width the layout allows for it.
        let mut canvas = Canvas::new();
        speaker(&mut canvas, 0, 0, Some(100), WHITE);
        assert!((0..240).all(|y| canvas.get(SPEAKER_WIDTH as i32, y) == BLACK));
    }
}
