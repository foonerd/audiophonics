//! The burn-in policy.
//!
//! A streamer is left on around the clock and an OLED showing largely
//! static content wears where it is lit, so brightness follows use, not the
//! clock on the wall. Full brightness after a touch; dim once `dim_after`
//! has passed with no touch; dark once `blank_after` has passed, but only
//! while nothing is playing: what is playing stays readable at the dim
//! level. Playback starting counts as a touch, because it is one, made from
//! another surface. A track changing does not, or a long playlist would
//! flash the panel to full brightness every few minutes.
//!
//! A tier set to zero is disabled.

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Full,
    Dim,
    Blank,
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Level::Full => "full",
            Level::Dim => "dim",
            Level::Blank => "blank",
        })
    }
}

#[derive(Debug)]
pub struct Idle {
    since: Instant,
    playing: bool,
    dim_after: Duration,
    blank_after: Duration,
}

impl Idle {
    pub fn new(now: Instant, dim_after: Duration, blank_after: Duration) -> Self {
        Self {
            since: now,
            playing: false,
            dim_after,
            blank_after,
        }
    }

    /// New tiers, as the settings changed. What counts as the last touch
    /// stays as it was.
    pub fn retime(&mut self, dim_after: Duration, blank_after: Duration) {
        self.dim_after = dim_after;
        self.blank_after = blank_after;
    }

    /// Somebody touched the panel.
    pub fn activity(&mut self, now: Instant) {
        self.since = now;
    }

    /// What the player is doing. Only the transition into playing counts as
    /// activity.
    pub fn player(&mut self, now: Instant, playing: bool) {
        if playing && !self.playing {
            self.since = now;
        }
        self.playing = playing;
    }

    pub fn level(&self, now: Instant) -> Level {
        let idle = now.saturating_duration_since(self.since);
        if !self.blank_after.is_zero() && idle >= self.blank_after && !self.playing {
            Level::Blank
        } else if !self.dim_after.is_zero() && idle >= self.dim_after {
            Level::Dim
        } else {
            Level::Full
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIM: Duration = Duration::from_secs(120);
    const BLANK: Duration = Duration::from_secs(600);

    fn at(t0: Instant, secs: u64) -> Instant {
        t0 + Duration::from_secs(secs)
    }

    #[test]
    fn full_then_dim_then_blank_when_nothing_plays() {
        let t0 = Instant::now();
        let idle = Idle::new(t0, DIM, BLANK);
        assert_eq!(idle.level(at(t0, 119)), Level::Full);
        assert_eq!(idle.level(at(t0, 120)), Level::Dim);
        assert_eq!(idle.level(at(t0, 599)), Level::Dim);
        assert_eq!(idle.level(at(t0, 600)), Level::Blank);
    }

    #[test]
    fn playing_dims_but_never_blanks() {
        let t0 = Instant::now();
        let mut idle = Idle::new(t0, DIM, BLANK);
        idle.player(t0, true);
        assert_eq!(idle.level(at(t0, 3600)), Level::Dim);
        idle.player(at(t0, 3600), false);
        assert_eq!(idle.level(at(t0, 3600)), Level::Blank);
    }

    #[test]
    fn a_touch_restores_full_and_restarts_the_clock() {
        let t0 = Instant::now();
        let mut idle = Idle::new(t0, DIM, BLANK);
        assert_eq!(idle.level(at(t0, 700)), Level::Blank);
        idle.activity(at(t0, 700));
        assert_eq!(idle.level(at(t0, 700)), Level::Full);
        assert_eq!(idle.level(at(t0, 819)), Level::Full);
        assert_eq!(idle.level(at(t0, 820)), Level::Dim);
    }

    #[test]
    fn playback_starting_is_a_touch_but_a_track_change_is_not() {
        let t0 = Instant::now();
        let mut idle = Idle::new(t0, DIM, BLANK);
        assert_eq!(idle.level(at(t0, 700)), Level::Blank);
        idle.player(at(t0, 700), true);
        assert_eq!(idle.level(at(t0, 700)), Level::Full);
        // Still playing, next track: nothing changes.
        idle.player(at(t0, 900), true);
        assert_eq!(idle.level(at(t0, 900)), Level::Dim);
    }

    #[test]
    fn zero_disables_a_tier() {
        let t0 = Instant::now();
        let no_dim = Idle::new(t0, Duration::ZERO, BLANK);
        assert_eq!(no_dim.level(at(t0, 599)), Level::Full);
        assert_eq!(no_dim.level(at(t0, 600)), Level::Blank);
        let no_blank = Idle::new(t0, DIM, Duration::ZERO);
        assert_eq!(no_blank.level(at(t0, 86_400)), Level::Dim);
        let never = Idle::new(t0, Duration::ZERO, Duration::ZERO);
        assert_eq!(never.level(at(t0, 86_400)), Level::Full);
    }

    #[test]
    fn levels_read_as_journal_words() {
        assert_eq!(Level::Blank.to_string(), "blank");
    }
}
