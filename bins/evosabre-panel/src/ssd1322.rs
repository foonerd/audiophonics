//! The SSD1322 behind the EVO Sabre's second display, over 4-wire SPI:
//! spidev for the bus, two GPIO lines for D/C and reset, taken through the
//! character device by chip label so the numbering differences between the
//! Pi 4 and Pi 5 families never matter.
//!
//! Contrast is set through the 8-bit contrast-current register (0xC1) with
//! the master current (0xC7) at full, so the configured value has 256 steps.

use crate::config::Config;
use crate::frame::{self, Frame};
use crate::grey;
use audiophonics_wire as wire;
use gpio_cdev::LineHandle;
use spidev::{SpiModeFlags, Spidev, SpidevOptions};
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

const CONSUMER: &str = "evosabre-panel";

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

        let mut chip = wire::open_chip(cfg.gpio_chip_label.as_deref())?;
        let dc = wire::output(&mut chip, cfg.dc_gpio, 0, "dc", CONSUMER)?;
        let rst = wire::output(&mut chip, cfg.reset_gpio, 1, "reset", CONSUMER)?;

        let chunk = wire::transfer_size();

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
        self.command(0xB8, &grey::TICKS)?; // grey scale table
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
