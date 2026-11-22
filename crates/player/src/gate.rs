//! The gate between the player answering and the player being ready.
//!
//! A player answers a request for its state as soon as its web server is
//! up, which is well before it has finished starting: its plugins are still
//! loading, and what it says of the track and the volume is not yet what it
//! will say. The player has a word of its own for where it is, and the page
//! of the track is kept back until that word is that it is ready.
//!
//! A player that has no such word, or says that its start failed, is not
//! waited for: there is nothing to wait on. One that goes on saying it is
//! starting is waited for only so long.

use crate::Readiness;
use std::time::{Duration, Instant};

/// How often the player is asked while it has not said it is ready.
pub const ASK_EVERY: Duration = Duration::from_secs(1);
/// The longest a player that answers is waited for.
pub const LONGEST: Duration = Duration::from_secs(120);

/// What became of the gate at an answer, where that is worth a line in the
/// journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opened {
    /// The player said it is ready.
    Ready,
    /// The player has no word for it.
    Unsaid,
    /// The player said its start failed.
    Failed,
    /// The player went on starting for longer than is waited.
    GaveUp,
}

#[derive(Debug, Clone, Copy)]
pub struct Gate {
    open: bool,
    /// When the player that is being waited for first answered.
    since: Option<Instant>,
    ask_at: Instant,
}

impl Gate {
    pub fn new(now: Instant) -> Self {
        Self {
            open: false,
            since: None,
            ask_at: now,
        }
    }

    /// True once the player that answers has said it is ready, or is no
    /// longer waited for.
    pub fn open(&self) -> bool {
        self.open
    }

    /// The player went: the next one that answers is waited for afresh.
    pub fn shut(&mut self, now: Instant) {
        self.open = false;
        self.since = None;
        self.ask_at = now;
    }

    /// True when the player is to be asked where it is.
    pub fn due(&self, now: Instant) -> bool {
        !self.open && now >= self.ask_at
    }

    /// When the player is next asked, while the gate is shut.
    pub fn ask_at(&self) -> Option<Instant> {
        (!self.open).then_some(self.ask_at)
    }

    /// What the player said of itself, or `None` where it gave no answer.
    /// Returns how the gate opened, if this opened it.
    pub fn heard(&mut self, said: Option<Readiness>, now: Instant) -> Option<Opened> {
        if self.open {
            return None;
        }
        let since = *self.since.get_or_insert(now);
        self.ask_at = now + ASK_EVERY;
        let opened = match said {
            Some(Readiness::Ready) => Opened::Ready,
            Some(Readiness::Unsaid) => Opened::Unsaid,
            Some(Readiness::Failed) => Opened::Failed,
            Some(Readiness::Starting) | None if now.saturating_duration_since(since) >= LONGEST => {
                Opened::GaveUp
            }
            Some(Readiness::Starting) | None => return None,
        };
        self.open = true;
        Some(opened)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    #[test]
    fn a_player_that_is_starting_is_waited_for_until_it_says_it_is_ready() {
        let t0 = Instant::now();
        let mut gate = Gate::new(t0);
        assert!(!gate.open() && gate.due(t0));
        assert_eq!(gate.heard(Some(Readiness::Starting), t0), None);
        assert!(!gate.open());
        // Asked again a second on, not before.
        assert!(!gate.due(t0 + SECOND / 2));
        assert!(gate.due(t0 + SECOND));
        assert_eq!(gate.ask_at(), Some(t0 + SECOND));
        assert_eq!(
            gate.heard(Some(Readiness::Starting), t0 + SECOND * 40),
            None
        );
        assert_eq!(
            gate.heard(Some(Readiness::Ready), t0 + SECOND * 41),
            Some(Opened::Ready)
        );
        assert!(gate.open());
        // Open: nothing more is asked, and nothing more is said.
        assert!(!gate.due(t0 + SECOND * 60));
        assert_eq!(gate.ask_at(), None);
        assert_eq!(
            gate.heard(Some(Readiness::Starting), t0 + SECOND * 60),
            None
        );
        assert!(gate.open());
    }

    #[test]
    fn a_player_that_is_ready_already_opens_it_at_the_first_answer() {
        let t0 = Instant::now();
        let mut gate = Gate::new(t0);
        assert_eq!(gate.heard(Some(Readiness::Ready), t0), Some(Opened::Ready));
        assert!(gate.open());
    }

    #[test]
    fn a_player_with_no_word_for_it_or_whose_start_failed_is_not_waited_for() {
        let t0 = Instant::now();
        let mut gate = Gate::new(t0);
        assert_eq!(
            gate.heard(Some(Readiness::Unsaid), t0),
            Some(Opened::Unsaid)
        );
        let mut gate = Gate::new(t0);
        assert_eq!(
            gate.heard(Some(Readiness::Failed), t0),
            Some(Opened::Failed)
        );
        assert!(gate.open());
    }

    #[test]
    fn a_player_that_never_says_it_is_ready_is_waited_for_only_so_long() {
        let t0 = Instant::now();
        let mut gate = Gate::new(t0);
        // The wait is counted from its first answer, said or not.
        assert_eq!(gate.heard(None, t0), None);
        assert_eq!(
            gate.heard(Some(Readiness::Starting), t0 + LONGEST - SECOND),
            None
        );
        assert_eq!(gate.heard(None, t0 + LONGEST), Some(Opened::GaveUp));
        assert!(gate.open());
    }

    #[test]
    fn a_player_that_went_is_waited_for_afresh_when_it_comes_back() {
        let t0 = Instant::now();
        let mut gate = Gate::new(t0);
        gate.heard(Some(Readiness::Ready), t0);
        let gone = t0 + SECOND * 300;
        gate.shut(gone);
        assert!(!gate.open() && gate.due(gone));
        // Its wait begins when it answers again, not when the first did.
        assert_eq!(
            gate.heard(Some(Readiness::Starting), gone + SECOND * 30),
            None
        );
        assert_eq!(
            gate.heard(
                Some(Readiness::Starting),
                gone + SECOND * 30 + LONGEST - SECOND
            ),
            None
        );
        assert_eq!(
            gate.heard(Some(Readiness::Ready), gone + SECOND * 80),
            Some(Opened::Ready)
        );
    }
}
