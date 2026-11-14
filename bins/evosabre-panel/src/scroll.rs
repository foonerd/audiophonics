//! A line of text wider than the panel, moved so all of it can be read:
//! held at the start, glided left, held at the end, then back to the start.
//! Moving the brightest pixels is also what the panel prefers.

use std::time::{Duration, Instant};

const HOLD: Duration = Duration::from_millis(2000);
/// Pixels per second. One pixel per frame at the 25 Hz the loop runs while
/// a line scrolls.
pub const SPEED: u32 = 25;

#[derive(Debug, Default)]
pub struct Marquee {
    text: String,
    text_px: u32,
    avail_px: u32,
    since: Option<Instant>,
}

impl Marquee {
    /// The text to show and the room it has. A change of text restarts the
    /// cycle; the same text keeps its place.
    pub fn set(&mut self, text: &str, text_px: u32, avail_px: u32, now: Instant) {
        if text != self.text || avail_px != self.avail_px {
            self.text = text.to_string();
            self.text_px = text_px;
            self.avail_px = avail_px;
            self.since = Some(now);
        }
    }

    pub fn scrolling(&self) -> bool {
        self.text_px > self.avail_px
    }

    /// How far left the text sits right now, in pixels.
    pub fn offset(&self, now: Instant) -> i32 {
        if !self.scrolling() {
            return 0;
        }
        let travel = self.text_px - self.avail_px;
        let travel_ms = u64::from(travel) * 1000 / u64::from(SPEED);
        let hold = HOLD.as_millis() as u64;
        let cycle = hold + travel_ms + hold;
        let t = self
            .since
            .map_or(0, |s| now.saturating_duration_since(s).as_millis() as u64)
            % cycle;
        if t < hold {
            0
        } else if t < hold + travel_ms {
            ((t - hold) * u64::from(SPEED) / 1000) as i32
        } else {
            travel as i32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_that_fits_never_moves() {
        let t0 = Instant::now();
        let mut m = Marquee::default();
        m.set("So Far Away", 99, 255, t0);
        assert!(!m.scrolling());
        assert_eq!(m.offset(t0 + Duration::from_secs(30)), 0);
    }

    #[test]
    fn hold_glide_hold_then_round_again() {
        let t0 = Instant::now();
        let mut m = Marquee::default();
        // 270 px in 255: 15 px of travel, 600 ms at 25 px/s.
        m.set(&"x".repeat(30), 270, 255, t0);
        assert!(m.scrolling());
        assert_eq!(m.offset(t0), 0);
        assert_eq!(
            m.offset(t0 + Duration::from_millis(1999)),
            0,
            "held at the start"
        );
        assert_eq!(m.offset(t0 + Duration::from_millis(2400)), 10, "gliding");
        assert_eq!(m.offset(t0 + Duration::from_millis(2600)), 15, "arrived");
        assert_eq!(
            m.offset(t0 + Duration::from_millis(4000)),
            15,
            "held at the end"
        );
        assert_eq!(m.offset(t0 + Duration::from_millis(4600)), 0, "round again");
    }

    #[test]
    fn a_new_text_restarts_and_the_same_text_keeps_its_place() {
        let t0 = Instant::now();
        let mut m = Marquee::default();
        m.set(&"x".repeat(30), 270, 255, t0);
        let later = t0 + Duration::from_millis(2400);
        m.set(&"x".repeat(30), 270, 255, later);
        assert_eq!(m.offset(later), 10, "unchanged text keeps scrolling");
        m.set(&"y".repeat(30), 270, 255, later);
        assert_eq!(m.offset(later), 0, "new text starts over");
    }
}
