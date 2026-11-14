//! What the handset's buttons do on a RaspDac Mini.
//!
//! Here the streamer acts on every button: the DAC board has no receiver
//! of its own. A button acts once however long it is held, except the four
//! that step, the two seeks and the two volumes, which step on while they
//! are held; and Option, which is two buttons in one: let go at once it
//! turns repeat over, held it turns shuffle over. Which of the two it is
//! cannot be known when it goes down, so a short press of Option acts a
//! moment after the button is let go.

use audiophonics_player::{Ask, Seek, Transport};
use audiophonics_remote::{Key, Press};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Transport(Transport),
    Seek(Seek),
    Ask(Ask),
    /// The DAC's next input.
    Input,
    /// The DAC's next filter.
    Filter,
    /// The second page, or back to the first.
    Page,
    /// Shut the player down.
    PowerOff,
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Action::Transport(Transport::Toggle) => "play or pause",
            Action::Transport(Transport::Next) => "next track",
            Action::Transport(Transport::Previous) => "previous track",
            Action::Transport(Transport::Stop) => "stop",
            Action::Seek(Seek::Forward) => "seek forward",
            Action::Seek(Seek::Back) => "seek back",
            Action::Ask(Ask::VolumeUp) => "volume up",
            Action::Ask(Ask::VolumeDown) => "volume down",
            Action::Ask(Ask::Mute) => "mute or unmute",
            Action::Ask(Ask::Repeat) => "repeat on or off",
            Action::Ask(Ask::Shuffle) => "shuffle on or off",
            Action::Input => "the DAC's next input",
            Action::Filter => "the DAC's next filter",
            Action::Page => "the other page",
            Action::PowerOff => "shut down",
        })
    }
}

/// A held seek or volume button steps on once it has been held this many
/// repeats...
const STEP_AFTER: u32 = 2;
/// ...and then on every other: about four steps a second.
const STEP_EVERY: u32 = 2;
/// Option held for this many repeats is the long press: about half a
/// second.
const HELD: u32 = 5;
/// A button whose frames have stopped for this long has been let go. The
/// handset repeats about nine times a second.
const LET_GO: Duration = Duration::from_millis(280);

/// Option, from when it went down until it is known which press it was.
#[derive(Debug, Clone, Copy)]
struct Pending {
    last_frame: Instant,
    /// The long press has acted; nothing more is due.
    acted: bool,
}

#[derive(Debug, Default)]
pub struct Keys {
    option: Option<Pending>,
}

impl Keys {
    /// What a frame from the handset asks for, if anything.
    pub fn press(&mut self, press: &Press, now: Instant) -> Option<Action> {
        let fresh = press.repeat == 0;
        let stepping = fresh
            || (press.repeat > STEP_AFTER
                && (press.repeat - STEP_AFTER).is_multiple_of(STEP_EVERY));
        match press.key? {
            Key::Play if fresh => Some(Action::Transport(Transport::Toggle)),
            Key::Right if fresh => Some(Action::Transport(Transport::Next)),
            Key::Left if fresh => Some(Action::Transport(Transport::Previous)),
            Key::Ok if fresh => Some(Action::Transport(Transport::Stop)),
            Key::PlayRight if stepping => Some(Action::Seek(Seek::Forward)),
            Key::PlayLeft if stepping => Some(Action::Seek(Seek::Back)),
            Key::VolumeUp if stepping => Some(Action::Ask(Ask::VolumeUp)),
            Key::VolumeDown if stepping => Some(Action::Ask(Ask::VolumeDown)),
            Key::Mute if fresh => Some(Action::Ask(Ask::Mute)),
            Key::Input if fresh => Some(Action::Input),
            Key::Up if fresh => Some(Action::Filter),
            Key::Down if fresh => Some(Action::Page),
            Key::Power if fresh => Some(Action::PowerOff),
            Key::Option => self.option(press.repeat, now),
            _ => None,
        }
    }

    fn option(&mut self, repeat: u32, now: Instant) -> Option<Action> {
        if repeat == 0 {
            self.option = Some(Pending {
                last_frame: now,
                acted: false,
            });
            return None;
        }
        let pending = self.option.as_mut()?;
        pending.last_frame = now;
        if !pending.acted && repeat >= HELD {
            pending.acted = true;
            return Some(Action::Ask(Ask::Shuffle));
        }
        None
    }

    /// When to ask [`Keys::due`] again: while Option is down.
    pub fn deadline(&self) -> Option<Instant> {
        self.option.map(|pending| pending.last_frame + LET_GO)
    }

    /// What has come due with time alone: Option let go before it counted
    /// as held.
    pub fn due(&mut self, now: Instant) -> Option<Action> {
        let pending = self.option?;
        if now.saturating_duration_since(pending.last_frame) < LET_GO {
            return None;
        }
        self.option = None;
        (!pending.acted).then_some(Action::Ask(Ask::Repeat))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: Key, repeat: u32) -> Press {
        Press {
            scancode: 0,
            key: Some(key),
            repeat,
        }
    }

    #[test]
    fn every_button_of_the_handset_does_something() {
        let now = Instant::now();
        let mut keys = Keys::default();
        for (key, action) in [
            (Key::Play, Action::Transport(Transport::Toggle)),
            (Key::Right, Action::Transport(Transport::Next)),
            (Key::Left, Action::Transport(Transport::Previous)),
            (Key::Ok, Action::Transport(Transport::Stop)),
            (Key::PlayRight, Action::Seek(Seek::Forward)),
            (Key::PlayLeft, Action::Seek(Seek::Back)),
            (Key::VolumeUp, Action::Ask(Ask::VolumeUp)),
            (Key::VolumeDown, Action::Ask(Ask::VolumeDown)),
            (Key::Mute, Action::Ask(Ask::Mute)),
            (Key::Input, Action::Input),
            (Key::Up, Action::Filter),
            (Key::Down, Action::Page),
            (Key::Power, Action::PowerOff),
        ] {
            assert_eq!(keys.press(&press(key, 0), now), Some(action), "{key}");
            assert!(!action.to_string().is_empty());
        }
        // Option answers when it is known which press it was.
        assert_eq!(keys.press(&press(Key::Option, 0), now), None);
        // A frame that is not this handset's asks for nothing.
        let foreign = Press {
            scancode: 0x1234,
            key: None,
            repeat: 0,
        };
        assert_eq!(keys.press(&foreign, now), None);
    }

    #[test]
    fn a_button_held_acts_once_and_the_four_that_step_go_on_stepping() {
        let now = Instant::now();
        let mut keys = Keys::default();
        for key in [
            Key::Play,
            Key::Right,
            Key::Ok,
            Key::Mute,
            Key::Input,
            Key::Up,
            Key::Down,
            Key::Power,
        ] {
            for repeat in 1..20 {
                assert_eq!(keys.press(&press(key, repeat), now), None, "{key} {repeat}");
            }
        }
        for key in [
            Key::PlayRight,
            Key::PlayLeft,
            Key::VolumeUp,
            Key::VolumeDown,
        ] {
            let stepped: Vec<u32> = (0..12)
                .filter(|&repeat| keys.press(&press(key, repeat), now).is_some())
                .collect();
            assert_eq!(stepped, [0, 4, 6, 8, 10], "{key}");
        }
    }

    #[test]
    fn option_let_go_at_once_is_repeat_a_moment_later() {
        let t0 = Instant::now();
        let mut keys = Keys::default();
        assert_eq!(keys.deadline(), None);
        assert_eq!(keys.press(&press(Key::Option, 0), t0), None);
        assert_eq!(keys.deadline(), Some(t0 + LET_GO));
        assert_eq!(
            keys.due(t0 + Duration::from_millis(100)),
            None,
            "too soon to know"
        );
        // A repeat or two, as a press of ordinary length sends.
        let t1 = t0 + Duration::from_millis(110);
        assert_eq!(keys.press(&press(Key::Option, 1), t1), None);
        assert_eq!(keys.due(t1 + Duration::from_millis(200)), None);
        assert_eq!(keys.due(t1 + LET_GO), Some(Action::Ask(Ask::Repeat)));
        assert_eq!(keys.due(t1 + LET_GO * 2), None, "once");
        assert_eq!(keys.deadline(), None);
    }

    #[test]
    fn option_held_is_shuffle_once_and_nothing_when_it_is_let_go() {
        let t0 = Instant::now();
        let mut keys = Keys::default();
        let at = |repeat: u32| t0 + Duration::from_millis(110) * repeat;
        assert_eq!(keys.press(&press(Key::Option, 0), at(0)), None);
        let mut acted = Vec::new();
        for repeat in 1..15 {
            assert_eq!(keys.due(at(repeat)), None, "still down at {repeat}");
            if let Some(action) = keys.press(&press(Key::Option, repeat), at(repeat)) {
                acted.push((repeat, action));
            }
        }
        assert_eq!(acted, [(HELD, Action::Ask(Ask::Shuffle))]);
        assert_eq!(keys.due(at(14) + LET_GO), None, "the long press has acted");
        assert_eq!(keys.deadline(), None);
        // A repeat with no press before it, the first frame lost: nothing.
        assert_eq!(keys.press(&press(Key::Option, 7), at(30)), None);
        assert_eq!(keys.due(at(40)), None);
    }
}
