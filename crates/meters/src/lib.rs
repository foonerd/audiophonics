//! The level meters: what the tap measured, given the movement of a meter.
//!
//! The tap's numbers are exact and jump with every hop; a bar drawn from
//! them as they come flickers. A meter rises at once and falls at a pace
//! the eye can follow. Each channel has three marks, in decibels below
//! full scale:
//!
//! - the level, the stream's RMS averaged over about a third of a second,
//!   as a VU meter averages: how loud it is;
//! - the peak, which jumps to the highest sample and falls back at a steady
//!   pace: how close to full scale it comes;
//! - the hold, the highest peak of the last moment, standing still for a
//!   while before it falls: the mark that shows a peak too brief to see.

pub mod levels;

use levels::Level;
use std::time::{Duration, Instant};

/// The bottom of the scale: sixty decibels under full scale.
pub const FLOOR_DB: f32 = -60.0;
/// The level's averaging time: a VU meter's.
const LEVEL_SECONDS: f32 = 0.3;
/// How fast the peak falls back.
const PEAK_FALL_DB_PER_SECOND: f32 = 20.0;
/// How long the hold stands before it falls, and how fast it falls then.
const HOLD: Duration = Duration::from_millis(1500);
const HOLD_FALL_DB_PER_SECOND: f32 = 30.0;
/// No hop for this long is a stream standing still: the marks fall.
const QUIET_AFTER: Duration = Duration::from_millis(200);
/// The longest step taken at once, so a meter that was not fed for a while
/// does not jump.
const LONGEST_STEP: f32 = 0.25;

/// One channel's marks, each as a place along the scale: 0.0 at the floor,
/// 1.0 at full scale.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Bar {
    pub level: f32,
    pub peak: f32,
    pub hold: f32,
}

#[derive(Debug, Clone, Copy)]
struct Channel {
    /// The mean square the level is the root of.
    square: f32,
    peak_db: f32,
    hold_db: f32,
    held_at: Option<Instant>,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            square: 0.0,
            peak_db: FLOOR_DB,
            hold_db: FLOOR_DB,
            held_at: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct Meters {
    channels: [Channel; 2],
    fed_at: Option<Instant>,
    heard_at: Option<Instant>,
    /// The last levels heard, which stand until the next hop or the quiet.
    last: Level,
}

/// A linear amplitude in decibels, no lower than the floor.
fn db(amplitude: f32) -> f32 {
    if amplitude <= 0.0 {
        return FLOOR_DB;
    }
    (20.0 * amplitude.log10()).clamp(FLOOR_DB, 0.0)
}

/// Where a level in decibels stands along the scale.
pub fn place(level_db: f32) -> f32 {
    ((level_db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}

impl Meters {
    /// Move the marks on to `now`. `heard` is what the tap published since
    /// the last call, if anything.
    pub fn feed(&mut self, heard: Option<Level>, now: Instant) {
        let step = self
            .fed_at
            .map_or(0.0, |at| now.saturating_duration_since(at).as_secs_f32())
            .min(LONGEST_STEP);
        self.fed_at = Some(now);
        if let Some(level) = heard {
            self.last = level;
            self.heard_at = Some(now);
        } else if self
            .heard_at
            .is_none_or(|at| now.saturating_duration_since(at) >= QUIET_AFTER)
        {
            self.last = Level::default();
        }

        let blend = 1.0 - (-step / LEVEL_SECONDS).exp();
        for (i, ch) in self.channels.iter_mut().enumerate() {
            let rms = self.last.rms[i].clamp(0.0, 1.0);
            ch.square += (rms * rms - ch.square) * blend;

            // The peak of a hop counts once, when it is heard.
            let peak_db = if heard.is_some() {
                db(self.last.peak[i])
            } else {
                FLOOR_DB
            };
            ch.peak_db = (ch.peak_db - PEAK_FALL_DB_PER_SECOND * step).max(peak_db);
            ch.peak_db = ch.peak_db.max(FLOOR_DB);

            if ch.peak_db >= ch.hold_db {
                ch.hold_db = ch.peak_db;
                ch.held_at = Some(now);
            } else if ch
                .held_at
                .is_none_or(|at| now.saturating_duration_since(at) >= HOLD)
            {
                ch.hold_db = (ch.hold_db - HOLD_FALL_DB_PER_SECOND * step).max(ch.peak_db);
            }
        }
    }

    pub fn bars(&self) -> [Bar; 2] {
        self.channels.map(|ch| Bar {
            level: place(db(ch.square.sqrt())),
            peak: place(ch.peak_db),
            hold: place(ch.hold_db),
        })
    }

    /// Every mark at the floor: nothing left to draw moving.
    pub fn at_rest(&self) -> bool {
        self.bars()
            .iter()
            .all(|bar| bar.level <= 0.0 && bar.peak <= 0.0 && bar.hold <= 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(peak: f32, rms: f32) -> Level {
        Level {
            peak: [peak, peak],
            rms: [rms, rms],
        }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn the_scale_runs_from_the_floor_to_full_scale_in_decibels() {
        assert_eq!(place(FLOOR_DB), 0.0);
        assert_eq!(place(0.0), 1.0);
        assert!(close(place(-30.0), 0.5));
        assert_eq!(place(-90.0), 0.0);
        assert!(close(db(0.5), -6.02));
        assert_eq!(db(0.0), FLOOR_DB);
    }

    #[test]
    fn the_peak_rises_at_once_and_falls_at_its_pace() {
        let mut m = Meters::default();
        let t0 = Instant::now();
        m.feed(Some(level(0.5, 0.1)), t0);
        assert!(close(m.bars()[0].peak, place(-6.02)), "{:?}", m.bars()[0]);

        // Half a second with quieter hops: ten decibels down.
        let mut t = t0;
        for _ in 0..10 {
            t += Duration::from_millis(50);
            m.feed(Some(level(0.001, 0.001)), t);
        }
        assert!(close(m.bars()[0].peak, place(-16.02)), "{:?}", m.bars()[0]);
    }

    #[test]
    fn the_hold_stands_while_the_peak_falls_then_follows_it_down() {
        let mut m = Meters::default();
        let t0 = Instant::now();
        m.feed(Some(level(1.0, 0.2)), t0);
        let mut t = t0;
        for _ in 0..20 {
            t += Duration::from_millis(50);
            m.feed(Some(level(0.01, 0.01)), t);
        }
        // One second on: the peak has fallen twenty decibels, the hold none.
        let bar = m.bars()[0];
        assert!(close(bar.peak, place(-20.0)), "{bar:?}");
        assert_eq!(bar.hold, 1.0);
        for _ in 0..20 {
            t += Duration::from_millis(50);
            m.feed(Some(level(0.01, 0.01)), t);
        }
        // Two seconds on: the hold let go half a second ago.
        let bar = m.bars()[0];
        assert!(bar.hold < 1.0 && bar.hold >= bar.peak, "{bar:?}");
    }

    #[test]
    fn the_level_is_an_average_that_settles_on_a_steady_tone() {
        let mut m = Meters::default();
        let mut t = Instant::now();
        m.feed(Some(level(0.25, 0.177)), t);
        assert!(m.bars()[0].level < place(-20.0), "not there at once");
        for _ in 0..80 {
            t += Duration::from_millis(25);
            m.feed(Some(level(0.25, 0.177)), t);
        }
        assert!(close(m.bars()[0].level, place(-15.04)), "{:?}", m.bars()[0]);
        assert!(m.bars()[0].level < m.bars()[0].peak);
    }

    #[test]
    fn a_level_raised_past_full_scale_stands_at_full_scale() {
        let mut m = Meters::default();
        let mut t = Instant::now();
        let loud = level(0.9, 0.7).gained(12.0);
        for _ in 0..80 {
            m.feed(Some(loud), t);
            t += Duration::from_millis(25);
        }
        let bar = m.bars()[0];
        assert_eq!(bar.peak, 1.0);
        assert_eq!(bar.hold, 1.0);
        assert!(bar.level > 0.99 && bar.level <= 1.0, "{bar:?}");
    }

    #[test]
    fn a_stream_standing_still_lets_every_mark_fall_to_rest() {
        let mut m = Meters::default();
        let mut t = Instant::now();
        m.feed(Some(level(0.8, 0.5)), t);
        assert!(!m.at_rest());
        // Hops stop: for a moment the last levels stand, then all falls.
        t += Duration::from_millis(100);
        m.feed(None, t);
        assert!(m.bars()[0].level > 0.0);
        for _ in 0..200 {
            t += Duration::from_millis(40);
            m.feed(None, t);
        }
        assert!(m.at_rest(), "{:?}", m.bars());
    }

    #[test]
    fn a_long_gap_between_feeds_is_taken_as_a_short_one() {
        let mut m = Meters::default();
        let t0 = Instant::now();
        m.feed(Some(level(1.0, 1.0)), t0);
        m.feed(Some(level(1.0, 1.0)), t0 + Duration::from_secs(60));
        assert_eq!(m.bars()[0].peak, 1.0);
        m.feed(None, t0 + Duration::from_secs(120));
        assert!(
            m.bars()[0].peak > place(-6.0),
            "one step, not a minute's fall"
        );
    }
}
