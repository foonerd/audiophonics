//! The SSD1322 behind the EVO Sabre's second display, over 4-wire SPI:
//! spidev for the bus, two GPIO lines for D/C and reset, taken through the
//! character device by chip label so the numbering differences between the
//! Pi 4 and Pi 5 families never matter.
//!
//! Contrast is set through the 8-bit contrast-current register (0xC1) with
//! the master current (0xC7) at full, so the configured value has 256 steps.

use crate::config::Config;
use crate::frame::{self, Frame};
use gpio_cdev::{Chip, Line, LineHandle, LineRequestFlags};
use spidev::{SpiModeFlags, Spidev, SpidevOptions};
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

const CONSUMER: &str = "evosabre-panel";

/// The grey scale: how long each of the fifteen levels above black is
/// driven, in ticks of the controller's display clock. From the second
/// level up this is the controller's own linear table, eight ticks a level.
/// That table leaves the first level dark, so the faintest grey it has is
/// the second. Here the first is driven for two ticks, a quarter of the
/// second: one grey fainter than the linear table can show, and every other
/// level as it was.
pub const GREYS: [u8; 15] = [2, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112];
const BUFSIZ_PARAM: &str = "/sys/module/spidev/parameters/bufsiz";
const DEFAULT_BUFSIZ: usize = 4096;

/// First and last RAM column for a 256-pixel-wide panel on this controller:
/// its RAM is 480 columns of four pixels, and the panel sits in the middle.
const COLUMN_FIRST: u8 = 0x1C;
const COLUMN_LAST: u8 = 0x5B;

pub struct Ssd1322 {
    spi: Spidev,
    dc: LineHandle,
    rst: LineHandle,
    chunk: usize,
    description: String,
}

pub type Result<T> = std::result::Result<T, String>;

impl Ssd1322 {
    /// Open the bus and the two lines, reset the controller and run its
    /// initialisation. Returns with the display on and cleared.
    pub fn open(cfg: &Config) -> Result<Self> {
        let mut spi =
            Spidev::open(&cfg.spi_device).map_err(|e| format!("{}: {e}", cfg.spi_device))?;
        spi.configure(
            &SpidevOptions::new()
                .bits_per_word(8)
                .max_speed_hz(cfg.spi_speed_hz)
                .mode(SpiModeFlags::SPI_MODE_0)
                .build(),
        )
        .map_err(|e| format!("{}: configure: {e}", cfg.spi_device))?;

        let mut chip = open_chip(cfg.gpio_chip_label.as_deref())?;
        let dc = request_output(&mut chip, cfg.dc_gpio, 0, "dc")?;
        let rst = request_output(&mut chip, cfg.reset_gpio, 1, "reset")?;

        let chunk = std::fs::read_to_string(BUFSIZ_PARAM)
            .ok()
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(DEFAULT_BUFSIZ)
            .max(64);

        let description = format!(
            "{} at {} Hz, dc gpio {} and reset gpio {} on {} ({}), {} byte transfers",
            cfg.spi_device,
            cfg.spi_speed_hz,
            cfg.dc_gpio,
            cfg.reset_gpio,
            chip.name(),
            chip.label(),
            chunk
        );

        let mut panel = Self {
            spi,
            dc,
            rst,
            chunk,
            description,
        };
        panel.reset()?;
        panel.init(cfg.contrast)?;
        panel.flush(&Frame::new())?;
        Ok(panel)
    }

    pub fn describe(&self) -> &str {
        &self.description
    }

    fn reset(&mut self) -> Result<()> {
        self.rst.set_value(0).map_err(gpio("reset low"))?;
        sleep(Duration::from_millis(1));
        self.rst.set_value(1).map_err(gpio("reset high"))?;
        sleep(Duration::from_millis(50));
        Ok(())
    }

    fn init(&mut self, contrast: u8) -> Result<()> {
        self.command(0xFD, &[0x12])?; // unlock commands
        self.command(0xAE, &[])?; // display off
        self.command(0xB3, &[0x91])?; // clock divider / oscillator
        self.command(0xCA, &[0x3F])?; // multiplex ratio: 64 rows
        self.command(0xA2, &[0x00])?; // display offset
        self.command(0xA1, &[0x00])?; // start line
        self.command(0xA0, &[0x14, 0x11])?; // remap: horizontal, nibble swap, dual COM
        self.command(0xB5, &[0x00])?; // GPIO pins off
        self.command(0xAB, &[0x01])?; // internal VDD regulator
        self.command(0xB4, &[0xA0, 0xFD])?; // display enhancement A: external VSL
        self.command(0xC1, &[contrast])?; // contrast current
        self.command(0xC7, &[0x0F])?; // master current: full
        self.command(0xB8, &GREYS)?; // grey scale table
        self.command(0x00, &[])?; // and the command that puts it to use
        self.command(0xB1, &[0xE2])?; // phase length
        self.command(0xD1, &[0x82, 0x20])?; // display enhancement B
        self.command(0xBB, &[0x1F])?; // precharge voltage
        self.command(0xB6, &[0x08])?; // second precharge period
        self.command(0xBE, &[0x07])?; // VCOMH
        self.command(0xA6, &[])?; // normal display
        self.command(0xA9, &[])?; // partial mode off
        sleep(Duration::from_millis(10));
        self.command(0xAF, &[])?; // display on
        sleep(Duration::from_millis(10));
        Ok(())
    }

    /// Write the whole frame. The controller's write pointer survives the
    /// chip-select gaps between transfers, so the frame goes out in pieces
    /// no larger than spidev's buffer.
    pub fn flush(&mut self, frame: &Frame) -> Result<()> {
        self.command(0x15, &[COLUMN_FIRST, COLUMN_LAST])?;
        self.command(0x75, &[0x00, (frame::HEIGHT - 1) as u8])?;
        self.command(0x5C, &[])?;
        self.dc.set_value(1).map_err(gpio("dc"))?;
        for piece in frame.bytes().chunks(self.chunk) {
            self.spi
                .write_all(piece)
                .map_err(|e| format!("spi write: {e}"))?;
        }
        Ok(())
    }

    pub fn set_contrast(&mut self, contrast: u8) -> Result<()> {
        self.command(0xC1, &[contrast])
    }

    /// Display off. The controller keeps its RAM and draws no current into
    /// the panel until `wake`.
    pub fn sleep(&mut self) -> Result<()> {
        self.command(0xAE, &[])
    }

    pub fn wake(&mut self) -> Result<()> {
        self.command(0xAF, &[])
    }

    fn command(&mut self, cmd: u8, data: &[u8]) -> Result<()> {
        self.dc.set_value(0).map_err(gpio("dc"))?;
        self.spi
            .write_all(&[cmd])
            .map_err(|e| format!("spi command {cmd:#04x}: {e}"))?;
        if !data.is_empty() {
            self.dc.set_value(1).map_err(gpio("dc"))?;
            self.spi
                .write_all(data)
                .map_err(|e| format!("spi data for {cmd:#04x}: {e}"))?;
        }
        Ok(())
    }
}

fn gpio(what: &'static str) -> impl Fn(gpio_cdev::Error) -> String {
    move |e| format!("gpio {what}: {e}")
}

/// The chip to take D/C and reset from. With a configured label it must
/// match exactly; without one, the SoC's pin controller is the chip whose
/// label starts with "pinctrl-" — pinctrl-bcm2711 on the Pi 4 family,
/// pinctrl-rp1 on the Pi 5 family — whatever number it was given.
fn open_chip(label: Option<&str>) -> Result<Chip> {
    let mut seen = Vec::new();
    for chip in gpio_cdev::chips()
        .map_err(|e| format!("listing gpio chips: {e}"))?
        .filter_map(std::result::Result::ok)
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

fn request_output(chip: &mut Chip, offset: u32, initial: u8, what: &str) -> Result<LineHandle> {
    let line = chip
        .get_line(offset)
        .map_err(|e| format!("gpio {offset} ({what}): {e}"))?;
    line.request(LineRequestFlags::OUTPUT, initial, CONSUMER)
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

    /// The controller takes a table only if each level is driven longer
    /// than the one before, and none for more than 180 ticks.
    #[test]
    fn the_grey_scale_rises_and_keeps_the_linear_table_from_the_second_level() {
        assert!(GREYS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(GREYS.iter().all(|&ticks| ticks <= 180));
        assert!(GREYS[0] > 0, "the first level is lit");
        for (i, &ticks) in GREYS.iter().enumerate().skip(1) {
            assert_eq!(usize::from(ticks), 8 * i, "level {}", i + 1);
        }
    }

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
}
