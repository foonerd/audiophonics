//! What the handset's buttons do on an EVO Sabre.
//!
//! The DAC board reads the handset itself for the volume, the mute, the
//! input and the power; those buttons are not the streamer's to act on. The
//! streamer takes the five that move through the music: play, the two
//! skips, and the two seeks.

use audiophonics_player::{Seek, Transport};
use audiophonics_remote::{Key, Press};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Transport(Transport),
    Seek(Seek),
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
        })
    }
}

/// A held seek button steps on once it has been held this many repeats...
const SEEK_AFTER: u32 = 2;
/// ...and then on every third: about three steps a second.
const SEEK_EVERY: u32 = 3;

/// What a frame from the handset asks of the player, if anything. A skip or
/// a play acts once however long the button is held; a seek steps on while
/// it is.
pub fn action(press: &Press) -> Option<Action> {
    let fresh = press.repeat == 0;
    let stepping = fresh
        || (press.repeat > SEEK_AFTER && (press.repeat - SEEK_AFTER).is_multiple_of(SEEK_EVERY));
    match press.key? {
        Key::Play if fresh => Some(Action::Transport(Transport::Toggle)),
        Key::Right if fresh => Some(Action::Transport(Transport::Next)),
        Key::Left if fresh => Some(Action::Transport(Transport::Previous)),
        Key::Up if stepping => Some(Action::Seek(Seek::Forward)),
        Key::Down if stepping => Some(Action::Seek(Seek::Back)),
        _ => None,
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
    fn the_five_keys_that_move_through_the_music_are_obeyed() {
        assert_eq!(
            action(&press(Key::Play, 0)),
            Some(Action::Transport(Transport::Toggle))
        );
        assert_eq!(
            action(&press(Key::Right, 0)),
            Some(Action::Transport(Transport::Next))
        );
        assert_eq!(
            action(&press(Key::Left, 0)),
            Some(Action::Transport(Transport::Previous))
        );
        assert_eq!(
            action(&press(Key::Up, 0)),
            Some(Action::Seek(Seek::Forward))
        );
        assert_eq!(action(&press(Key::Down, 0)), Some(Action::Seek(Seek::Back)));
    }

    #[test]
    fn the_dac_boards_own_keys_are_left_to_it() {
        for key in [
            Key::Power,
            Key::Input,
            Key::Option,
            Key::Ok,
            Key::PlayLeft,
            Key::PlayRight,
            Key::VolumeDown,
            Key::VolumeUp,
            Key::Mute,
        ] {
            assert_eq!(action(&press(key, 0)), None, "{key}");
        }
        let foreign = Press {
            scancode: 0x1234,
            key: None,
            repeat: 0,
        };
        assert_eq!(action(&foreign), None);
    }

    #[test]
    fn a_held_skip_acts_once_and_a_held_seek_steps_on() {
        for repeat in 1..12 {
            assert_eq!(action(&press(Key::Play, repeat)), None);
            assert_eq!(action(&press(Key::Right, repeat)), None);
        }
        let stepped: Vec<u32> = (0..12)
            .filter(|&r| action(&press(Key::Up, r)).is_some())
            .collect();
        assert_eq!(stepped, vec![0, 5, 8, 11]);
    }
}
