//! Seven-segment numerals for the clock, drawn as an instrument would show
//! them: chamfered bars, a one-pixel gap at every joint, and the unlit
//! segments left as a faint ghost, which a real LED digit cannot do and
//! which makes the figure read as a display rather than as type.
//!
//! One digit is 22 x 48 with four-pixel segments. "14:07" is 108 wide.

use crate::frame::Frame;

pub const DIGIT_W: u32 = 22;
pub const DIGIT_H: u32 = 48;
const T: u32 = 4;
const GAP: u32 = 4;
const COLON_W: u32 = T;

/// Segments a to g as bits 0 to 6: a top, b upper right, c lower right,
/// d bottom, e lower left, f upper left, g middle.
pub const MASKS: [u8; 10] = [
    0b0111111, // 0: a b c d e f
    0b0000110, // 1: b c
    0b1011011, // 2: a b d e g
    0b1001111, // 3: a b c d g
    0b1100110, // 4: b c f g
    0b1101101, // 5: a c d f g
    0b1111101, // 6: a c d e f g
    0b0000111, // 7: a b c
    0b1111111, // 8
    0b1101111, // 9: a b c d f g
];

/// A horizontal bar from x0 to x1 inclusive, T rows from y, chamfered one
/// pixel at each end on its outer rows.
fn bar_h(f: &mut Frame, x0: u32, x1: u32, y: u32, luma: u8) {
    for r in 0..T {
        let inset = u32::from(r == 0 || r == T - 1);
        for x in (x0 + inset)..=(x1 - inset) {
            f.set(x, y + r, luma);
        }
    }
}

/// A vertical bar from y0 to y1 inclusive, T columns from x, chamfered one
/// pixel at each end on its outer columns.
fn bar_v(f: &mut Frame, x: u32, y0: u32, y1: u32, luma: u8) {
    for c in 0..T {
        let inset = u32::from(c == 0 || c == T - 1);
        for y in (y0 + inset)..=(y1 - inset) {
            f.set(x + c, y, luma);
        }
    }
}

fn segment(f: &mut Frame, x: u32, y: u32, which: u8, luma: u8) {
    let (w, h) = (DIGIT_W, DIGIT_H);
    let mid = h / 2 - T / 2; // the middle bar's top row: 22
    match which {
        0 => bar_h(f, x + T + 1, x + w - T - 2, y, luma),
        1 => bar_v(f, x + w - T, y + T + 1, y + mid - 2, luma),
        2 => bar_v(f, x + w - T, y + mid + T + 1, y + h - T - 2, luma),
        3 => bar_h(f, x + T + 1, x + w - T - 2, y + h - T, luma),
        4 => bar_v(f, x, y + mid + T + 1, y + h - T - 2, luma),
        5 => bar_v(f, x, y + T + 1, y + mid - 2, luma),
        _ => bar_h(f, x + T + 1, x + w - T - 2, y + mid, luma),
    }
}

/// One digit: every segment as a ghost, then the lit ones over it.
pub fn digit(f: &mut Frame, x: u32, y: u32, d: u8, lit: u8, ghost: u8) {
    let mask = MASKS[usize::from(d.min(9))];
    for s in 0..7u8 {
        let on = mask & (1 << s) != 0;
        if on || ghost > 0 {
            segment(f, x, y, s, if on { lit } else { ghost });
        }
    }
}

fn colon(f: &mut Frame, x: u32, y: u32, luma: u8) {
    for (cy, _) in [(DIGIT_H / 4 - T / 2, ()), (DIGIT_H * 3 / 4 - T / 2, ())] {
        for dy in 0..T {
            for dx in 0..T {
                f.set(x + dx, y + cy + dy, luma);
            }
        }
    }
}

/// The width "hh:mm" takes.
#[cfg(test)]
pub fn width(s: &str) -> u32 {
    let mut w = 0;
    for c in s.chars() {
        w += match c {
            '0'..='9' => DIGIT_W + GAP,
            ':' => COLON_W + GAP,
            _ => 0,
        };
    }
    w.saturating_sub(GAP)
}

pub fn draw(f: &mut Frame, x: u32, y: u32, s: &str, lit: u8, ghost: u8) {
    let mut cx = x;
    for c in s.chars() {
        match c {
            '0'..='9' => {
                digit(f, cx, y, c as u8 - b'0', lit, ghost);
                cx += DIGIT_W + GAP;
            }
            ':' => {
                colon(f, cx, y, lit);
                cx += COLON_W + GAP;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_is_108_wide_and_48_tall() {
        assert_eq!(width("14:07"), 108);
        let mut f = Frame::new();
        draw(&mut f, 4, 12, "14:07", 12, 2);
        assert!(
            (0..64).all(|y| f.get(4 + 108, y) == 0),
            "nothing past the width"
        );
        assert!(
            (0..256).all(|x| f.get(x, 12 + 48) == 0),
            "nothing below the height"
        );
        assert!((0..256).all(|x| f.get(x, 11) == 0), "nothing above the top");
    }

    #[test]
    fn a_one_lights_the_right_bars_and_ghosts_the_rest() {
        let mut f = Frame::new();
        digit(&mut f, 0, 0, 1, 12, 2);
        // Upper right bar, full brightness, four wide with the chamfer.
        assert_eq!(f.get(19, 10), 12);
        assert_eq!(f.get(18, 10), 12);
        assert_eq!(f.get(21, 10), 12);
        assert_eq!(f.get(18, 5), 0, "chamfered corner");
        assert_eq!(f.get(19, 5), 12);
        // The top bar is a ghost, and the middle bar too.
        assert_eq!(f.get(10, 1), 2);
        assert_eq!(f.get(10, 23), 2);
        // A one-pixel gap between the top bar and the upper-right bar.
        assert_eq!(f.get(19, 4), 0);
    }

    #[test]
    fn an_eight_lights_everything_and_a_zero_leaves_the_middle_ghosted() {
        let mut f = Frame::new();
        digit(&mut f, 0, 0, 8, 12, 2);
        assert!([1u32, 23, 45].iter().all(|&y| f.get(10, y) == 12));
        let mut z = Frame::new();
        digit(&mut z, 0, 0, 0, 12, 2);
        assert_eq!(z.get(10, 23), 2);
        assert_eq!(z.get(10, 1), 12);
    }

    #[test]
    fn without_a_ghost_level_unlit_segments_are_dark() {
        let mut f = Frame::new();
        digit(&mut f, 0, 0, 1, 12, 0);
        assert_eq!(f.get(10, 1), 0);
    }
}
