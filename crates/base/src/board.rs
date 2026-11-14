//! Which Raspberry Pi this is, from what the firmware says of it.
//!
//! The device tree names the board in words (`model`) and lists what it is
//! compatible with, most specific first, ending in the name of its SoC.
//! The SoC is what matters to a daemon: the Pi 5 family (BCM2712) brings
//! its GPIO, SPI and I2S from a separate chip, the RP1, where the earlier
//! boards have them in the SoC itself. Code that can be written to work on
//! both is; this is for what cannot, and for saying in the journal which
//! board a report came from.

use std::path::Path;

/// The SoC families of the boards an Audiophonics unit is built on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Soc {
    /// Raspberry Pi 5, Compute Module 5.
    Bcm2712,
    /// Raspberry Pi 4, Pi 400, Compute Module 4.
    Bcm2711,
    /// Raspberry Pi 3 and Zero 2 W, and the Pi 2 revision built on it.
    Bcm2837,
    /// Raspberry Pi 2.
    Bcm2836,
    /// Raspberry Pi 1 and Zero.
    Bcm2835,
    /// Not a board this knows.
    Unknown,
}

impl std::fmt::Display for Soc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Soc::Bcm2712 => "BCM2712",
            Soc::Bcm2711 => "BCM2711",
            Soc::Bcm2837 => "BCM2837",
            Soc::Bcm2836 => "BCM2836",
            Soc::Bcm2835 => "BCM2835",
            Soc::Unknown => "unknown SoC",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    /// The board in words, such as "Raspberry Pi 5 Model B Rev 1.0".
    pub model: String,
    pub soc: Soc,
}

impl std::fmt::Display for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.model, self.soc)
    }
}

impl Board {
    /// True for the Pi 5 family, whose header pins belong to the RP1.
    pub fn has_rp1(&self) -> bool {
        self.soc == Soc::Bcm2712
    }
}

pub fn detect() -> Board {
    detect_in(Path::new("/proc/device-tree"))
}

pub fn detect_in(tree: &Path) -> Board {
    let read = |name: &str| std::fs::read(tree.join(name)).unwrap_or_default();
    let model = text(&read("model"));
    Board {
        model: if model.is_empty() {
            "unknown board".to_string()
        } else {
            model
        },
        soc: soc(&read("compatible")),
    }
}

/// A device-tree string: its bytes up to the terminating NUL.
fn text(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

/// The SoC named in a `compatible` property: NUL-separated strings, any of
/// which may name it.
fn soc(compatible: &[u8]) -> Soc {
    let names = [
        ("brcm,bcm2712", Soc::Bcm2712),
        ("brcm,bcm2711", Soc::Bcm2711),
        ("brcm,bcm2837", Soc::Bcm2837),
        ("brcm,bcm2836", Soc::Bcm2836),
        ("brcm,bcm2835", Soc::Bcm2835),
    ];
    compatible
        .split(|&b| b == 0)
        .find_map(|entry| {
            names
                .iter()
                .find(|(name, _)| entry == name.as_bytes())
                .map(|(_, soc)| *soc)
        })
        .unwrap_or(Soc::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(tag: &str, model: &[u8], compatible: &[u8]) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("audiophonics-board-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("model"), model).unwrap();
        std::fs::write(dir.join("compatible"), compatible).unwrap();
        dir
    }

    #[test]
    fn a_pi_5_and_a_pi_4_are_told_apart_by_their_soc() {
        let five = tree(
            "five",
            b"Raspberry Pi 5 Model B Rev 1.0\0",
            b"raspberrypi,5-model-b\0brcm,bcm2712\0",
        );
        let board = detect_in(&five);
        assert_eq!(board.model, "Raspberry Pi 5 Model B Rev 1.0");
        assert_eq!(board.soc, Soc::Bcm2712);
        assert!(board.has_rp1());
        assert_eq!(
            board.to_string(),
            "Raspberry Pi 5 Model B Rev 1.0 (BCM2712)"
        );

        let four = tree(
            "four",
            b"Raspberry Pi 4 Model B Rev 1.5\0",
            b"raspberrypi,4-model-b\0brcm,bcm2711\0",
        );
        let board = detect_in(&four);
        assert_eq!(board.soc, Soc::Bcm2711);
        assert!(!board.has_rp1());
        std::fs::remove_dir_all(&five).unwrap();
        std::fs::remove_dir_all(&four).unwrap();
    }

    #[test]
    fn the_compute_modules_and_the_older_boards_are_named_by_the_same_property() {
        assert_eq!(
            soc(b"raspberrypi,5-compute-module\0brcm,bcm2712\0"),
            Soc::Bcm2712
        );
        assert_eq!(
            soc(b"raspberrypi,4-compute-module\0brcm,bcm2711\0"),
            Soc::Bcm2711
        );
        assert_eq!(
            soc(b"raspberrypi,3-model-b-plus\0brcm,bcm2837\0"),
            Soc::Bcm2837
        );
        assert_eq!(soc(b"raspberrypi,2-model-b\0brcm,bcm2836\0"), Soc::Bcm2836);
        assert_eq!(
            soc(b"raspberrypi,model-zero-w\0brcm,bcm2835\0"),
            Soc::Bcm2835
        );
        assert_eq!(soc(b"radxa,rock-5b\0rockchip,rk3588\0"), Soc::Unknown);
        assert_eq!(soc(b""), Soc::Unknown);
    }

    #[test]
    fn a_machine_without_a_device_tree_is_an_unknown_board() {
        let board = detect_in(Path::new("/nonexistent/device-tree"));
        assert_eq!(board.model, "unknown board");
        assert_eq!(board.soc, Soc::Unknown);
        assert_eq!(board.to_string(), "unknown board (unknown SoC)");
    }
}
