//! What the audio tap measured, read from its ring.
//!
//! The tap (evotap) stands in the player's audio path and publishes, a hop
//! at a time, the peak and RMS of each channel into a ring under /dev/shm,
//! each hop at the moment its audio is heard. One ring is one stream: it
//! goes when the stream closes or changes shape, and the next has another.
//! This finds the live ring of the plugin's own tag, takes the hops that
//! arrived since the last look as one, and looks again when the ring goes.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The tag in the ring's file name, which the plugin's ALSA contribution
/// gives its tap. Another plugin's tap in the same chain has another.
pub const TAG: &str = "evosabre";
/// How often a ring is looked for while there is none.
const LOOK_EVERY: Duration = Duration::from_millis(500);

/// One look's worth of levels: linear, 1.0 being full scale.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Level {
    pub peak: [f32; 2],
    pub rms: [f32; 2],
}

impl Level {
    /// The same levels so many decibels up or down: the meters' gain. It
    /// moves what is shown, not what is played.
    pub fn gained(self, db: f32) -> Self {
        let by = 10f32.powf(db / 20.0);
        Self {
            peak: self.peak.map(|v| v * by),
            rms: self.rms.map(|v| v * by),
        }
    }
}

pub struct Source {
    dir: PathBuf,
    prefix: String,
    reader: Option<tap::Reader>,
    last_seq: u64,
    look_at: Instant,
    told: Option<bool>,
}

impl Source {
    pub fn new() -> Self {
        Self::under(Path::new(tap::ring::DIR))
    }

    pub fn under(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
            prefix: format!("{}{TAG}.", tap::ring::PREFIX),
            reader: None,
            last_seq: 0,
            look_at: Instant::now(),
            told: None,
        }
    }

    /// The hops published since the last look, as one: the highest peak
    /// and RMS among them, so a transient between two frames of the
    /// display still shows. `None` when there is no live ring, or nothing
    /// new in it: a stream that is not there, or one standing still.
    pub fn take(&mut self, now: Instant) -> Option<Level> {
        if self.reader.as_ref().is_some_and(|r| !r.is_live()) {
            self.reader = None;
        }
        if self.reader.is_none() && now >= self.look_at {
            self.look_at = now + LOOK_EVERY;
            self.reader = self.find();
            self.last_seq = 0;
        }
        let reader = self.reader.as_ref()?;
        let info = reader.info();
        if info.seq == self.last_seq {
            return None;
        }
        // Every hop since the last look, as far back as the ring holds.
        let first = if self.last_seq == 0
            || info.seq.saturating_sub(self.last_seq) >= u64::from(info.slots)
        {
            info.seq
        } else {
            self.last_seq + 1
        };
        self.last_seq = info.seq;
        let merged = tap::ring::merge_hops((first..=info.seq).filter_map(|seq| reader.slot(seq)))?;
        let mut level = Level {
            peak: merged.peak,
            rms: merged.rms,
        };
        if info.channels < 2 {
            level.peak[1] = level.peak[0];
            level.rms[1] = level.rms[0];
        }
        Some(level)
    }

    /// Whether a ring is being read, said once each time it changes, for
    /// the journal.
    pub fn changed(&mut self) -> Option<bool> {
        let reading = self.reader.is_some();
        if self.told == Some(reading) {
            return None;
        }
        self.told = Some(reading);
        Some(reading)
    }

    /// The ring this is reading, for the journal.
    pub fn path(&self) -> Option<&Path> {
        self.reader.as_ref().map(tap::Reader::path)
    }

    /// The live ring of this plugin's tag, the most recently written first.
    fn find(&self) -> Option<tap::Reader> {
        tap::Reader::find(&self.dir)
            .into_iter()
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&self.prefix))
            })
            .find_map(|path| tap::Reader::open(&path).ok().filter(tap::Reader::is_live))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tap::ring::{Frame, Writer};

    fn dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("evosabre-levels-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn hop(seq: u64, left: f32, right: f32) -> Frame {
        Frame {
            seq,
            time_ns: tap::ring::now_ns(),
            frames: seq * 1024,
            peak: [left, right],
            rms: [left / 2.0, right / 2.0],
            spectrum: [vec![0.0; 4], vec![0.0; 4]],
            hold: [vec![0.0; 4], vec![0.0; 4]],
            ..Frame::default()
        }
    }

    fn writer(dir: &Path, tag: &str, channels: u32) -> Writer {
        let demand = tap::Demand::new(4, channels as usize, tap::Scale::Log);
        Writer::create(dir, tag, 44_100, channels, demand, 1024, 16).unwrap()
    }

    #[test]
    fn the_hops_since_the_last_look_arrive_as_their_highest() {
        let dir = dir("merge");
        let mut w = writer(&dir, TAG, 2);
        let mut source = Source::under(&dir);
        let t0 = Instant::now();
        assert_eq!(source.take(t0), None, "nothing published yet");

        // A ring nothing was written to is not live yet; it is at the
        // next look.
        w.publish(&hop(1, 0.2, 0.1));
        let t1 = t0 + LOOK_EVERY;
        assert_eq!(
            source.take(t1).unwrap().peak,
            [0.2, 0.1],
            "the latest at first"
        );

        w.publish(&hop(2, 0.9, 0.3));
        w.publish(&hop(3, 0.4, 0.5));
        let level = source.take(t1).unwrap();
        assert_eq!(level.peak, [0.9, 0.5]);
        assert_eq!(level.rms, [0.45, 0.25]);
        assert_eq!(source.take(t1), None, "nothing new");

        w.publish(&hop(4, 0.1, 0.1));
        assert_eq!(source.take(t1).unwrap().peak, [0.1, 0.1]);
        assert_eq!(source.changed(), Some(true));
        assert_eq!(source.changed(), None, "said once");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn gain_moves_every_level_by_its_decibels() {
        let level = Level {
            peak: [0.5, 0.25],
            rms: [0.1, 0.0],
        };
        let up = level.gained(6.0);
        assert!((up.peak[0] - 0.9976).abs() < 0.001, "{up:?}");
        assert!((up.peak[1] - 0.4988).abs() < 0.001, "{up:?}");
        assert_eq!(up.rms[1], 0.0, "silence stays silence");
        let down = level.gained(-12.0);
        assert!((down.peak[0] - 0.1256).abs() < 0.001, "{down:?}");
        assert_eq!(level.gained(0.0), level);
    }

    #[test]
    fn another_plugins_ring_is_not_this_ones() {
        let dir = dir("tag");
        let mut other = writer(&dir, "glass", 2);
        other.publish(&hop(1, 0.8, 0.8));
        let mut source = Source::under(&dir);
        assert_eq!(source.take(Instant::now()), None);
        assert_eq!(source.changed(), Some(false));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_ring_that_appears_later_is_found_at_the_next_look() {
        let dir = dir("later");
        let mut source = Source::under(&dir);
        let t0 = Instant::now();
        assert_eq!(source.take(t0), None);
        let mut w = writer(&dir, TAG, 1);
        w.publish(&hop(1, 0.6, 0.0));
        assert_eq!(source.take(t0), None, "not looked for again yet");
        let level = source.take(t0 + LOOK_EVERY).unwrap();
        assert_eq!(level.peak, [0.6, 0.6], "one channel fills both");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
