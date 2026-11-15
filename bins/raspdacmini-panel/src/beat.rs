//! The beat that frames are drawn on.
//!
//! A line that runs across the glass looks even only if it moves the same
//! distance every frame, and frames come the same time apart. So what moves
//! is not drawn for the moment the loop happened to come round, which is a
//! little later each time by however long the last frame took: it is drawn
//! for a time on one even grid, and the loop sleeps until the grid's next
//! time, not for a frame's length from wherever it is.
//!
//! The loop also comes round between two beats, for the player's state or
//! a button. What it draws then is the frame of the beat it is in, which is
//! the frame already on the glass: nothing that moves moves between beats,
//! so nothing is sent to the panel out of step.

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

    /// The beat a frame drawn at `now` is drawn for: the one `now` is in.
    /// A loop that came round a little before a beat, by less than a
    /// quarter of the way from the last, came round for that beat.
    pub fn of(&self, now: Instant) -> Instant {
        let gone = now.saturating_duration_since(self.first).as_micros();
        let apart = self.apart.as_micros();
        self.at((gone + apart / 4) / apart)
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
    fn a_frame_is_drawn_for_the_beat_it_is_in() {
        let t0 = Instant::now();
        let beat = Beat::new(t0, APART);
        assert_eq!(beat.of(t0), t0);
        assert_eq!(beat.of(t0 + MS * 3), t0);
        // Between two beats: still the one that began.
        assert_eq!(beat.of(t0 + MS * 10), t0);
        assert_eq!(beat.of(t0 + MS * 14), t0);
        // A little before the next: that one.
        assert_eq!(beat.of(t0 + MS * 15), t0 + APART);
        assert_eq!(beat.of(t0 + MS * 19), t0 + APART);
        assert_eq!(beat.of(t0 + MS * 1004), t0 + APART * 50);
    }

    #[test]
    fn a_loop_that_comes_round_between_beats_draws_no_new_frame() {
        let t0 = Instant::now();
        let beat = Beat::new(t0, APART);
        let mut ticker = Ticker::new(t0);
        ticker.set("A long line", 460, 40, 320, t0);
        let on_the_beat = t0 + HOLD + APART * 7 + MS;
        let drawn = ticker.offset(beat.of(on_the_beat));
        for later in [3, 8, 13] {
            assert_eq!(ticker.offset(beat.of(on_the_beat + MS * later)), drawn);
        }
        assert_eq!(ticker.offset(beat.of(on_the_beat + APART)), drawn + 1);
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
        assert_eq!(beat.of(before), t0);
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
            let offset = ticker.offset(beat.of(woke));
            if let Some(last) = last {
                assert_eq!(offset, last + 1, "frame {frame}");
            }
            last = Some(offset);
        }
    }
}
