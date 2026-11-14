//! The player watched for the journal: what changed, said once.

use crate::{Result, State};

/// One line for the journal, and whether it is bad news.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    Changed(String),
    Outage(String),
}

/// How many polls in a row may fail before the player counts as gone. A
/// busy player misses one now and then; what it last said stands meanwhile.
pub const TOLERATED_MISSES: u32 = 2;

/// Turns a stream of poll results into a stream of journal lines.
///
/// Nothing is said while nothing changes: a panel polling twice a second must
/// not fill the journal. An outage is reported once, when it begins, and the
/// first state after it is always reported, because whatever the player says
/// on return is news even when it matches what was said before.
#[derive(Debug, Default)]
pub struct Probe {
    last: Option<State>,
    /// What was last said about the state; the seek position moves every
    /// poll and is not part of it, so an unchanged track stays silent.
    said: Option<String>,
    /// Polls that have failed since the last that did not.
    misses: u32,
    down: bool,
}

impl Probe {
    /// The last state the player gave, if it is answering.
    pub fn state(&self) -> Option<&State> {
        self.last.as_ref()
    }

    pub fn observe(&mut self, polled: Result<State>) -> Option<Report> {
        match polled {
            Ok(state) => {
                self.down = false;
                self.misses = 0;
                let line = describe(&state);
                let same = self.said.as_deref() == Some(line.as_str());
                self.last = Some(state);
                if same {
                    return None;
                }
                self.said = Some(line.clone());
                Some(Report::Changed(line))
            }
            Err(e) => {
                if self.last.is_some() && self.misses < TOLERATED_MISSES {
                    self.misses += 1;
                    return None;
                }
                self.last = None;
                self.said = None;
                if self.down {
                    return None;
                }
                self.down = true;
                Some(Report::Outage(format!("player unreachable: {e}")))
            }
        }
    }
}

fn describe(s: &State) -> String {
    let status = if s.status.is_empty() { "-" } else { &s.status };
    let volume = match s.volume {
        Some(v) if s.mute => format!("{v} muted"),
        Some(v) => v.to_string(),
        None => "-".into(),
    };
    format!(
        "status={status} volume={volume} title={:?} artist={:?} album={:?} format={:?}",
        s.title,
        s.artist,
        s.album,
        format!("{} {}", s.samplerate, s.bitdepth).trim(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    fn playing(volume: u8) -> State {
        State {
            status: "play".into(),
            title: "Aria".into(),
            artist: "Gould".into(),
            album: "Goldberg".into(),
            volume: Some(volume),
            mute: false,
            samplerate: "44.1 kHz".into(),
            bitdepth: "16 bit".into(),
            ..State::default()
        }
    }

    #[test]
    fn the_first_state_is_reported_in_full() {
        let mut p = Probe::default();
        assert_eq!(
            p.observe(Ok(playing(67))),
            Some(Report::Changed(
                "status=play volume=67 title=\"Aria\" artist=\"Gould\" album=\"Goldberg\" format=\"44.1 kHz 16 bit\"".into()
            ))
        );
    }

    #[test]
    fn an_unchanged_state_is_silent_and_a_change_is_not() {
        let mut p = Probe::default();
        p.observe(Ok(playing(67)));
        assert_eq!(p.observe(Ok(playing(67))), None);
        let mut moved = playing(67);
        moved.seek = Some(5_000);
        assert_eq!(
            p.observe(Ok(moved)),
            None,
            "the seek moving is not a change worth a line"
        );
        let mut muted = playing(67);
        muted.mute = true;
        assert!(
            matches!(p.observe(Ok(muted)), Some(Report::Changed(l)) if l.contains("volume=67 muted"))
        );
    }

    #[test]
    fn an_outage_is_reported_once() {
        let mut p = Probe::default();
        p.observe(Ok(playing(67)));
        for _ in 0..TOLERATED_MISSES {
            assert_eq!(p.observe(Err(Error::Decode("cut short".into()))), None);
            assert!(p.state().is_some(), "what it last said stands");
        }
        assert_eq!(
            p.observe(Err(Error::Decode("cut short".into()))),
            Some(Report::Outage(
                "player unreachable: decode: cut short".into()
            ))
        );
        assert!(p.state().is_none());
        assert_eq!(p.observe(Err(Error::Decode("cut short".into()))), None);
        assert_eq!(p.observe(Err(Error::Decode("x".into()))), None);
    }

    #[test]
    fn the_first_state_after_an_outage_is_reported_even_if_unchanged() {
        let mut p = Probe::default();
        p.observe(Ok(playing(67)));
        for _ in 0..=TOLERATED_MISSES {
            p.observe(Err(Error::Decode("cut short".into())));
        }
        assert!(p.observe(Ok(playing(67))).is_some());
        assert_eq!(p.observe(Ok(playing(67))), None);
    }

    #[test]
    fn a_poll_that_fails_now_and_then_is_not_an_outage_and_says_nothing() {
        let mut p = Probe::default();
        p.observe(Ok(playing(67)));
        for _ in 0..10 {
            assert_eq!(p.observe(Err(Error::Decode("cut short".into()))), None);
            assert_eq!(p.observe(Ok(playing(67))), None, "the same state, unbroken");
        }
        // A player that never answered has nothing to stand on: gone at once.
        let mut cold = Probe::default();
        assert!(matches!(
            cold.observe(Err(Error::Decode("cut short".into()))),
            Some(Report::Outage(_))
        ));
    }

    #[test]
    fn an_empty_player_reads_as_dashes() {
        let mut p = Probe::default();
        assert_eq!(
            p.observe(Ok(State::default())),
            Some(Report::Changed(
                "status=- volume=- title=\"\" artist=\"\" album=\"\" format=\"\"".into()
            ))
        );
    }
}
