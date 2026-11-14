//! Glyphs the fonts do not have, drawn at the panel's scale: nine rows tall
//! to sit inside a ten-pixel text row with a one-pixel foot.

use crate::frame::Frame;

pub struct Icon {
    pub rows: &'static [&'static str],
}

impl Icon {
    pub fn width(&self) -> u32 {
        self.rows.iter().map(|r| r.len()).max().unwrap_or(0) as u32
    }
}

pub const PLAY: Icon = Icon {
    rows: &[
        "XX....", "XXX...", "XXXX..", "XXXXX.", "XXXXXX", "XXXXX.", "XXXX..", "XXX...", "XX....",
    ],
};

pub const PAUSE: Icon = Icon {
    rows: &[
        "XX.XX", "XX.XX", "XX.XX", "XX.XX", "XX.XX", "XX.XX", "XX.XX", "XX.XX", "XX.XX",
    ],
};

pub const STOP: Icon = Icon {
    rows: &[
        ".......", "XXXXXXX", "XXXXXXX", "XXXXXXX", "XXXXXXX", "XXXXXXX", "XXXXXXX", "XXXXXXX",
        ".......",
    ],
};

pub const SPEAKER: Icon = Icon {
    rows: &[
        "...X......",
        "..XX..X...",
        ".XXX...X..",
        "XXXX.X..X.",
        "XXXX..X.X.",
        "XXXX.X..X.",
        ".XXX...X..",
        "..XX..X...",
        "...X......",
    ],
};

pub const SPEAKER_OFF: Icon = Icon {
    rows: &[
        "...X......",
        "..XX......",
        ".XXX.X..X.",
        "XXXX..XX..",
        "XXXX...X..",
        "XXXX..XX..",
        ".XXX.X..X.",
        "..XX......",
        "...X......",
    ],
};

/// The queue, or the track, plays again at its end.
pub const REPEAT: Icon = Icon {
    rows: &[
        "......X..",
        ".XXXXXXX.",
        "X.....X.X",
        "X.......X",
        "X.......X",
        "X.......X",
        "X.X.....X",
        ".XXXXXXX.",
        "..X......",
    ],
};

/// The queue plays in a shuffled order.
pub const RANDOM: Icon = Icon {
    rows: &[
        ".........",
        ".......X.",
        "XX...XXXX",
        "..X.X..X.",
        "...X.....",
        "..X.X..X.",
        "XX...XXXX",
        ".......X.",
        ".........",
    ],
};

pub fn draw(frame: &mut Frame, icon: &Icon, x: u32, y: u32, luma: u8) {
    for (dy, row) in icon.rows.iter().enumerate() {
        for (dx, c) in row.bytes().enumerate() {
            if c == b'X' {
                frame.set(x + dx as u32, y + dy as u32, luma);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_is_nine_rows_of_equal_width() {
        for icon in [
            &PLAY,
            &PAUSE,
            &STOP,
            &SPEAKER,
            &SPEAKER_OFF,
            &REPEAT,
            &RANDOM,
        ] {
            assert_eq!(icon.rows.len(), 9);
            let w = icon.width() as usize;
            assert!(icon.rows.iter().all(|r| r.len() == w));
            assert!(icon
                .rows
                .iter()
                .all(|r| r.bytes().all(|b| b == b'X' || b == b'.')));
        }
    }

    #[test]
    fn draw_lands_where_asked() {
        let mut f = Frame::new();
        draw(&mut f, &PLAY, 3, 2, 12);
        assert_eq!(f.get(3, 2), 12);
        assert_eq!(f.get(8, 6), 12, "the widest row");
        assert_eq!(f.get(2, 2), 0);
    }
}
