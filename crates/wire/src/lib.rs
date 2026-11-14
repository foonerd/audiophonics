//! How a display is wired to the board: the GPIO lines beside its SPI bus.
//!
//! The lines are taken through the character device, from the chip found
//! by its label, so the numbering differences between the Pi 4 and Pi 5
//! families never matter. A line that cannot be taken is refused with the
//! name of whoever holds it.

use gpio_cdev::{Chip, Line, LineHandle, LineRequestFlags};

const BUFSIZ_PARAM: &str = "/sys/module/spidev/parameters/bufsiz";
const DEFAULT_BUFSIZ: usize = 4096;

/// The most bytes spidev takes in one transfer on this system.
pub fn transfer_size() -> usize {
    std::fs::read_to_string(BUFSIZ_PARAM)
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(DEFAULT_BUFSIZ)
        .max(64)
}

/// The chip to take a display's lines from. With a configured label it must
/// match exactly; without one, the SoC's pin controller is the chip whose
/// label starts with "pinctrl-" — pinctrl-bcm2711 on the Pi 4 family,
/// pinctrl-rp1 on the Pi 5 family — whatever number it was given.
pub fn open_chip(label: Option<&str>) -> Result<Chip, String> {
    let mut seen = Vec::new();
    for chip in gpio_cdev::chips()
        .map_err(|e| format!("listing gpio chips: {e}"))?
        .filter_map(Result::ok)
    {
        let found = chip.label().to_string();
        let wanted = match label {
            Some(want) => found == want,
            None => found.starts_with("pinctrl-"),
        };
        if wanted {
            return Ok(chip);
        }
        seen.push(format!("{} ({found})", chip.name()));
    }
    Err(format!(
        "no gpio chip {}; present: {}",
        match label {
            Some(want) => format!("labelled {want}"),
            None => "labelled pinctrl-*".to_string(),
        },
        if seen.is_empty() {
            "none".to_string()
        } else {
            seen.join(", ")
        }
    ))
}

/// Take a line as an output at a level, in the name of `consumer`. `what`
/// is what the line is for, for the refusal.
pub fn output(
    chip: &mut Chip,
    offset: u32,
    initial: u8,
    what: &str,
    consumer: &str,
) -> Result<LineHandle, String> {
    let line = chip
        .get_line(offset)
        .map_err(|e| format!("gpio {offset} ({what}): {e}"))?;
    line.request(LineRequestFlags::OUTPUT, initial, consumer)
        .map_err(|e| format!("gpio {offset} ({what}): request: {e}{}", holder(&line)))
}

/// Who has a line that could not be taken, as the kernel tells it.
fn holder(line: &Line) -> String {
    line.info()
        .map(|info| held_by(info.consumer(), info.is_used()))
        .unwrap_or_default()
}

/// What to add to a refusal. The kernel names whoever took a line; a line
/// exported through /sys/class/gpio is named "sysfs" and stays taken after
/// the program that exported it has gone.
fn held_by(consumer: Option<&str>, used: bool) -> String {
    match consumer {
        Some("sysfs") => "; the line was taken through /sys/class/gpio and not given back, \
                          and is free again once the player has restarted"
            .to_string(),
        Some(name) if !name.is_empty() => format!("; the line is held by {name}"),
        _ if used => "; the line is in use by the kernel".to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_line_names_whoever_holds_it() {
        let sysfs = held_by(Some("sysfs"), true);
        assert!(sysfs.contains("/sys/class/gpio"), "{sysfs}");
        assert!(sysfs.contains("restarted"), "{sysfs}");
        assert_eq!(
            held_by(Some("spi0 CS0"), true),
            "; the line is held by spi0 CS0"
        );
        assert_eq!(held_by(None, true), "; the line is in use by the kernel");
        assert_eq!(
            held_by(Some(""), true),
            "; the line is in use by the kernel"
        );
        assert_eq!(held_by(None, false), "", "nothing to add for a free line");
    }

    #[test]
    fn a_transfer_is_never_smaller_than_a_command_needs() {
        assert!(transfer_size() >= 64);
    }
}
