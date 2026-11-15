//! The beat that frames are drawn on.
//!
//! A line that runs across the glass looks even only if it moves the same
//! distance every frame, and frames come the same time apart. So what moves
//! is not drawn for the moment the loop happened to come round, which is a
//! little later each time by however long the last frame took: it is drawn
//! for a time on one even grid, and the loop sleeps until the grid's next
//! time, not for a frame's length from wherever it is.

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct Beat {
    first: Instant,
    apart: Duration,
}

impl Beat {
    /// A beat every `apart`, the first at `first`.
    pub fn new(first: Instant, apart: Duration) -> Self {
        Self { first, apart }
    }

    /// The beat nearest to `now`: the time a frame drawn now is drawn for.
    pub fn nearest(&self, now: Instant) -> Instant {
        let gone = now.saturating_duration_since(self.first).as_micros();
        let apart = self.apart.as_micros();
        self.at((gone + apart / 2) / apart)
    }

    /// The first beat after `now`: when what moves is drawn next.
    pub fn after(&self, now: Instant) -> Instant {
        let gone = now.saturating_duration_since(self.first).as_micros();
        self.at(gone / self.apart.as_micros() + 1)
    }

    fn at(&self, beats: u128) -> Instant {
        self.first + Duration::from_micros((beats * self.apart.as_micros()) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ticker::{Ticker, HOLD};

    const APART: Duration = Duration::from_millis(20);
    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn a_frame_drawn_a_little_late_or_early_is_drawn_for_its_beat() {
        let t0 = Instant::now();
        let beat = Beat::new(t0, APART);
        assert_eq!(beat.nearest(t0), t0);
        assert_eq!(beat.nearest(t0 + MS * 3), t0);
        assert_eq!(beat.nearest(t0 + MS * 9), t0);
        assert_eq!(beat.nearest(t0 + MS * 10), t0 + APART);
        assert_eq!(beat.nearest(t0 + MS * 19), t0 + APART);
        assert_eq!(beat.nearest(t0 + MS * 1004), t0 + APART * 50);
    }

    #[test]
    fn the_next_beat_is_always_ahead() {
        let t0 = Instant::now();
        let beat = Beat::new(t0, APART);
        assert_eq!(beat.after(t0), t0 + APART);
        assert_eq!(beat.after(t0 + MS * 19), t0 + APART);
        assert_eq!(beat.after(t0 + APART), t0 + APART * 2);
        assert_eq!(beat.after(t0 + MS * 47), t0 + APART * 3);
    }

    #[test]
    fn a_time_before_the_first_beat_is_the_first_beat() {
        let t0 = Instant::now() + Duration::from_secs(1);
        let beat = Beat::new(t0, APART);
        let before = t0 - Duration::from_millis(500);
        assert_eq!(beat.nearest(before), t0);
        assert_eq!(beat.after(before), t0 + APART);
    }

    /// The reason for the beat: a running line drawn at each beat, however
    /// late the loop came round to it, moves by the same step every frame.
    #[test]
    fn a_running_line_drawn_on_the_beat_moves_one_step_a_frame() {
        let t0 = Instant::now();
        let beat = Beat::new(t0, APART);
        let mut ticker = Ticker::new(t0);
        ticker.set("A long line", 460, 40, 320, t0);
        // The loop comes round up to nine milliseconds late, never the same.
        let lateness = [0u32, 7, 2, 9, 4, 1, 8, 3, 6, 5];
        let mut last = None;
        for frame in 0..500u32 {
            let woke = t0 + HOLD + APART * frame + MS * lateness[frame as usize % lateness.len()];
            let offset = ticker.offset(beat.nearest(woke));
            if let Some(last) = last {
                assert_eq!(offset, last + 1, "frame {frame}");
            }
            last = Some(offset);
        }
    }
}
