//! A line wider than the glass, moved so all of it can be read: held at its
//! start, then run to the left with itself following after a gap, round and
//! round, held again each time its start comes back.

use std::time::{Duration, Instant};

/// Pixels a second.
pub const SPEED: u32 = 50;
/// How long the line stands still at its start.
pub const HOLD: Duration = Duration::from_millis(1600);

#[derive(Debug, Clone)]
pub struct Ticker {
    text: String,
    /// The line's width and the gap before it comes round again; nought
    /// when the line fits and stays where it is.
    lap: u32,
    since: Instant,
}

impl Ticker {
    pub fn new(now: Instant) -> Self {
        Self {
            text: String::new(),
            lap: 0,
            since: now,
        }
    }

    /// Give the line that is shown, its width, the gap that follows it and
    /// the room there is. A line that is the one already shown keeps
    /// running from where it is; a new one starts from its beginning.
    pub fn set(&mut self, text: &str, text_px: u32, gap_px: u32, room_px: u32, now: Instant) {
        let lap = if text_px > room_px {
            text_px + gap_px
        } else {
            0
        };
        if text != self.text || lap != self.lap {
            self.text = text.to_string();
            self.lap = lap;
            self.since = now;
        }
    }

    pub fn running(&self) -> bool {
        self.lap > 0
    }

    /// How far left of its place the line is drawn at `now`.
    pub fn offset(&self, now: Instant) -> u32 {
        if self.lap == 0 {
            return 0;
        }
        // One round: the hold, then the lap at the ticker's speed.
        let run = Duration::from_millis(u64::from(self.lap) * 1000 / u64::from(SPEED));
        let round = HOLD + run;
        let into = now.saturating_duration_since(self.since).as_millis() % round.as_millis();
        let moving = into.saturating_sub(HOLD.as_millis());
        (moving * u128::from(SPEED) / 1000) as u32
    }

    /// How far the line is from coming round again: where its second copy
    /// is drawn, right of the first.
    pub fn lap(&self) -> u32 {
        self.lap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_that_fits_stays_where_it_is() {
        let t0 = Instant::now();
        let mut ticker = Ticker::new(t0);
        ticker.set("So Far Away", 150, 40, 320, t0);
        assert!(!ticker.running());
        assert_eq!(ticker.offset(t0 + Duration::from_secs(30)), 0);
        ticker.set("Just fits", 320, 40, 320, t0);
        assert!(!ticker.running());
    }

    #[test]
    fn a_wide_line_is_held_then_runs_and_is_held_again_when_it_comes_round() {
        let t0 = Instant::now();
        let mut ticker = Ticker::new(t0);
        ticker.set("A long line", 460, 40, 320, t0);
        assert!(ticker.running());
        assert_eq!(ticker.lap(), 500);
        assert_eq!(ticker.offset(t0), 0);
        assert_eq!(ticker.offset(t0 + HOLD), 0);
        assert_eq!(ticker.offset(t0 + HOLD + Duration::from_secs(1)), SPEED);
        assert_eq!(ticker.offset(t0 + HOLD + Duration::from_secs(9)), 450);
        // Ten seconds is the whole lap: the line is back and held.
        let round = HOLD + Duration::from_secs(10);
        assert_eq!(ticker.offset(t0 + round), 0);
        assert_eq!(ticker.offset(t0 + round + HOLD / 2), 0);
        assert_eq!(
            ticker.offset(t0 + round + HOLD + Duration::from_secs(2)),
            100
        );
    }

    #[test]
    fn the_same_line_keeps_running_and_a_new_one_starts_again() {
        let t0 = Instant::now();
        let mut ticker = Ticker::new(t0);
        ticker.set("A long line", 460, 40, 320, t0);
        let later = t0 + HOLD + Duration::from_secs(3);
        ticker.set("A long line", 460, 40, 320, later);
        assert_eq!(ticker.offset(later), 150);
        ticker.set("Another long line", 520, 40, 320, later);
        assert_eq!(ticker.offset(later), 0);
        assert_eq!(ticker.offset(later + HOLD + Duration::from_secs(1)), SPEED);
        // A clock that reads earlier than the start is the start.
        assert_eq!(ticker.offset(t0), 0);
    }
}
