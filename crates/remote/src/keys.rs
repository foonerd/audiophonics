//! The handset's buttons and the scancodes the kernel reports for them.
//!
//! The EVO Sabre and the RaspDac Mini LCD ship the same handset. Its frames
//! are NEC with a 16-bit address: four bytes, address, address, command and
//! the command inverted, each sent least significant bit first.

/// A button on the handset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Power,
    Input,
    Option,
    Up,
    Down,
    Left,
    Right,
    Ok,
    PlayLeft,
    Play,
    PlayRight,
    VolumeDown,
    VolumeUp,
    Mute,
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Key::Power => "power",
            Key::Input => "input",
            Key::Option => "option",
            Key::Up => "up",
            Key::Down => "down",
            Key::Left => "left",
            Key::Right => "right",
            Key::Ok => "ok",
            Key::PlayLeft => "play left",
            Key::Play => "play",
            Key::PlayRight => "play right",
            Key::VolumeDown => "volume down",
            Key::VolumeUp => "volume up",
            Key::Mute => "mute",
        })
    }
}

/// Each button's frame as it is transmitted, first bit sent in the most
/// significant place: the form a recording of the handset gives.
pub const FRAMES: [(Key, u32); 14] = [
    (Key::Power, 0x0198_08F7),
    (Key::Input, 0x0198_9867),
    (Key::Option, 0x0198_18E7),
    (Key::Up, 0x0198_E817),
    (Key::Down, 0x0198_6897),
    (Key::Left, 0x0198_F20D),
    (Key::Right, 0x0198_8A75),
    (Key::Ok, 0x0198_0AF5),
    (Key::PlayLeft, 0x0198_F807),
    (Key::Play, 0x0198_B847),
    (Key::PlayRight, 0x0198_58A7),
    (Key::VolumeDown, 0x0198_38C7),
    (Key::VolumeUp, 0x0198_629D),
    (Key::Mute, 0x0198_7887),
];

/// The scancode the kernel's NEC decoder reports for a transmitted frame.
///
/// The decoder turns each byte round, since it was sent least significant
/// bit first, and then names the frame by what its bytes are: plain NEC when
/// the second byte is the first inverted, extended NEC when it is not, and
/// 32-bit NEC when the command is not followed by its inverse.
pub const fn scancode(frame: u32) -> u32 {
    let address = ((frame >> 24) as u8).reverse_bits() as u32;
    let not_address = ((frame >> 16) as u8).reverse_bits() as u32;
    let command = ((frame >> 8) as u8).reverse_bits() as u32;
    let not_command = (frame as u8).reverse_bits() as u32;
    if command ^ not_command != 0xFF {
        not_address << 24 | address << 16 | not_command << 8 | command
    } else if address ^ not_address != 0xFF {
        address << 16 | not_address << 8 | command
    } else {
        address << 8 | command
    }
}

/// The button a scancode belongs to, if it is one of the handset's.
pub fn key(code: u32) -> Option<Key> {
    FRAMES
        .iter()
        .find(|(_, frame)| scancode(*frame) == code)
        .map(|(key, _)| *key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_handset_is_extended_nec_under_one_address() {
        assert_eq!(scancode(0x0198_08F7), 0x80_1910, "power");
        assert_eq!(scancode(0x0198_B847), 0x80_191D, "play");
        assert_eq!(scancode(0x0198_F20D), 0x80_194F, "left");
        assert_eq!(scancode(0x0198_629D), 0x80_1946, "volume up");
        for (key, frame) in FRAMES {
            assert_eq!(scancode(frame) >> 8, 0x8019, "{key}");
        }
    }

    #[test]
    fn every_button_has_its_own_scancode_and_comes_back_from_it() {
        for (i, (key, frame)) in FRAMES.iter().enumerate() {
            assert_eq!(super::key(scancode(*frame)), Some(*key));
            for (other, other_frame) in &FRAMES[i + 1..] {
                assert_ne!(
                    scancode(*frame),
                    scancode(*other_frame),
                    "{key} and {other}"
                );
            }
        }
        assert_eq!(key(0x80_1900), None);
        assert_eq!(key(0), None);
    }

    #[test]
    fn the_three_kinds_of_nec_frame_are_named_as_the_kernel_names_them() {
        // Address 0x04 and its inverse, command 0x08 and its inverse.
        assert_eq!(scancode(0x20DF_10EF), 0x0408, "plain");
        // The same command under a 16-bit address.
        assert_eq!(scancode(0x2012_10EF), 0x04_4808, "extended");
        // A command not followed by its inverse.
        assert_eq!(scancode(0x20DF_1012), 0xFB04_4808, "32-bit");
    }
}
