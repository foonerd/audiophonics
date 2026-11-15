//! The player watched for the journal: what changed, said once.

use crate::{Result, State};

/// One line for the journal, and whether it is bad news.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    Changed(String),
    Outage(String),
}

/// How many polls in a row may fail before the player counts as gone,
/// unless a display asks for more patience. A busy player misses one now
/// and then; what it last said stands meanwhile.
pub const TOLERATED_MISSES: u32 = 2;

/// Turns a stream of poll results into a stream of journal lines.
///
/// Nothing is said while nothing changes: a panel polling twice a second must
/// not fill the journal. An outage is reported once, when it begins, and the
/// first state after it is always reported, because whatever the player says
/// on return is news even when it matches what was said before.
#[derive(Debug)]
pub struct Probe {
    last: Option<State>,
    /// What was last said about the state; the seek position moves every
    /// poll and is not part of it, so an unchanged track stays silent.
    said: Option<String>,
    /// Polls that have failed since the last that did not.
    misses: u32,
    /// How many of them in a row are let pass.
    tolerated: u32,
    down: bool,
}

impl Default for Probe {
    fn default() -> Self {
        Self::tolerating(TOLERATED_MISSES)
    }
}

impl Probe {
    /// A probe that lets this many polls in a row fail before the player
    /// counts as gone: for a display whose picture of a player that is
    /// only slow is worth more than a prompt word that it has gone.
    pub fn tolerating(misses: u32) -> Self {
        Self {
            last: None,
            said: None,
            misses: 0,
            tolerated: misses,
            down: false,
        }
    }

    /// The last state the player gave, if it is answering.
    pub fn state(&self) -> Option<&State> {
        self.last.as_ref()
    }

    /// The player is gone, and it is known without waiting to see whether
    /// it answers the next poll: nothing listens where it should be.
    pub fn gone(&mut self, why: impl std::fmt::Display) -> Option<Report> {
        self.misses = self.tolerated;
        self.last = None;
        self.said = None;
        if self.down {
            return None;
        }
        self.down = true;
        Some(Report::Outage(format!("player unreachable: {why}")))
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
                if self.last.is_some() && self.misses < self.tolerated {
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
    fn a_patient_probe_lets_more_polls_fail_before_the_player_is_gone() {
        let mut p = Probe::tolerating(10);
        p.observe(Ok(playing(67)));
        for _ in 0..10 {
            assert_eq!(p.observe(Err(Error::Decode("slow".into()))), None);
            assert!(p.state().is_some());
        }
        assert!(matches!(
            p.observe(Err(Error::Decode("slow".into()))),
            Some(Report::Outage(_))
        ));
        assert!(p.state().is_none());
        // One answer in between starts the count again.
        let mut p = Probe::tolerating(3);
        p.observe(Ok(playing(67)));
        for _ in 0..5 {
            for _ in 0..3 {
                assert_eq!(p.observe(Err(Error::Decode("slow".into()))), None);
            }
            p.observe(Ok(playing(67)));
            assert!(p.state().is_some());
        }
    }

    #[test]
    fn a_player_known_to_be_gone_is_gone_at_once_and_said_once() {
        let mut p = Probe::tolerating(40);
        p.observe(Ok(playing(67)));
        assert_eq!(
            p.gone("connection refused"),
            Some(Report::Outage(
                "player unreachable: connection refused".into()
            ))
        );
        assert!(p.state().is_none());
        assert_eq!(p.gone("connection refused"), None, "said once");
        assert_eq!(p.observe(Err(Error::Decode("slow".into()))), None);
        // Its return is news.
        assert!(matches!(
            p.observe(Ok(playing(67))),
            Some(Report::Changed(_))
        ));
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
