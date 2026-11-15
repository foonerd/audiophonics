//! The ILI9341 behind the RaspDac Mini's display, over 4-wire SPI: spidev
//! for the bus and two GPIO lines for D/C and reset, taken from the chip
//! found by its label so the same program runs on the Pi 4 and Pi 5
//! families.
//!
//! The controller keeps the picture in its own memory. A part of the glass
//! is changed by naming a window of that memory and writing its pixels, so
//! what goes over the bus is only what changed.
//!
//! The panel is mounted on its side: the controller's 240 x 320 memory is
//! read out turned, as 320 x 240, and its colour filter is blue, green,
//! red. Both are in the one byte written to the address mode register.

use crate::canvas::{Area, Frame, HEIGHT, WIDTH};
use crate::handover;
use gpio_cdev::LineHandle;
use spidev::{SpiModeFlags, Spidev, SpidevOptions};
use std::io::Write;
use std::time::Duration;

const CONSUMER: &str = "raspdacmini-panel";

pub type Result<T> = std::result::Result<T, String>;

// The controller's commands, by the names its data sheet gives them.
const SOFT_RESET: u8 = 0x01;
const SLEEP_OUT: u8 = 0x11;
const GAMMA_SET: u8 = 0x26;
const DISPLAY_OFF: u8 = 0x28;
const DISPLAY_ON: u8 = 0x29;
const COLUMN_ADDRESS: u8 = 0x2A;
const PAGE_ADDRESS: u8 = 0x2B;
const MEMORY_WRITE: u8 = 0x2C;
const ADDRESS_MODE: u8 = 0x36;
const PIXEL_FORMAT: u8 = 0x3A;

/// The address mode of this panel: rows and columns exchanged and both
/// mirrored, which is the glass on its side with the header to the right,
/// and the colour order blue, green, red.
const ON_ITS_SIDE: u8 = 0x80 | 0x40 | 0x20 | 0x08;
/// Sixteen bits a pixel.
const SIXTEEN_BITS: u8 = 0x55;

/// What the controller is told at its start, after its reset and before it
/// is woken: each command with its data. The values are the panel maker's.
const START: &[(u8, &[u8])] = &[
    (DISPLAY_OFF, &[]),
    (0xCF, &[0x00, 0x83, 0x30]),             // power control B
    (0xED, &[0x64, 0x03, 0x12, 0x81]),       // power-on sequence
    (0xE8, &[0x85, 0x01, 0x79]),             // driver timing A
    (0xCB, &[0x39, 0x2C, 0x00, 0x34, 0x02]), // power control A
    (0xF7, &[0x20]),                         // pump ratio
    (0xEA, &[0x00, 0x00]),                   // driver timing B
    (0xC0, &[0x26]),                         // power control 1
    (0xC1, &[0x11]),                         // power control 2
    (0xC5, &[0x35, 0x3E]),                   // VCOM control 1
    (0xC7, &[0xBE]),                         // VCOM control 2
    (PIXEL_FORMAT, &[SIXTEEN_BITS]),
    (ADDRESS_MODE, &[ON_ITS_SIDE]),
    (0xB1, &[0x00, 0x1B]), // frame rate: 70 Hz
    (0xF2, &[0x08]),       // the third gamma: off
    (GAMMA_SET, &[0x01]),
    (
        0xE0, // positive gamma
        &[
            0x1F, 0x1A, 0x18, 0x0A, 0x0F, 0x06, 0x45, 0x87, 0x32, 0x0A, 0x07, 0x02, 0x07, 0x05,
            0x00,
        ],
    ),
    (
        0xE1, // negative gamma
        &[
            0x00, 0x25, 0x27, 0x05, 0x10, 0x09, 0x3A, 0x78, 0x4D, 0x05, 0x18, 0x0D, 0x38, 0x3A,
            0x1F,
        ],
    ),
    (0xB7, &[0x07]),                   // entry mode
    (0xB6, &[0x0A, 0x82, 0x27, 0x00]), // display function
];

/// What carries commands and pixels to the controller.
pub trait Wire {
    /// Pull the controller's reset line and let it go.
    fn reset(&mut self) -> Result<()>;
    /// One command and the data that belongs to it.
    fn command(&mut self, command: u8, data: &[u8]) -> Result<()>;
    /// Pixels for the window last named, following a memory write.
    fn pixels(&mut self, bytes: &[u8]) -> Result<()>;
    /// Let the controller have the time it needs.
    fn pause(&mut self, time: Duration);
}

pub struct Ili9341<W: Wire> {
    wire: W,
    lit: bool,
}

impl<W: Wire> Ili9341<W> {
    /// Reset the controller, run its start, fill its memory with black and
    /// only then switch the display on: the glass goes from nothing to a
    /// black screen, never through whatever the memory held.
    pub fn start(wire: W) -> Result<Self> {
        let mut panel = Self { wire, lit: false };
        panel.wire.reset()?;
        panel.wire.command(SOFT_RESET, &[])?;
        panel.wire.pause(Duration::from_millis(5));
        for (command, data) in START {
            panel.wire.command(*command, data)?;
        }
        panel.wire.command(SLEEP_OUT, &[])?;
        panel.wire.pause(Duration::from_millis(120));
        panel.show(&Frame::black(), Area::ALL)?;
        panel.light()?;
        Ok(panel)
    }

    /// Send one area of a frame: the window, then its pixels.
    pub fn show(&mut self, frame: &Frame, area: Area) -> Result<()> {
        if area.w == 0 || area.h == 0 || area.x + area.w > WIDTH || area.y + area.h > HEIGHT {
            return Err(format!("{area:?} is not an area of the glass"));
        }
        let span = |first: u32, count: u32| {
            let last = first + count - 1;
            [
                (first >> 8) as u8,
                first as u8,
                (last >> 8) as u8,
                last as u8,
            ]
        };
        self.wire.command(COLUMN_ADDRESS, &span(area.x, area.w))?;
        self.wire.command(PAGE_ADDRESS, &span(area.y, area.h))?;
        self.wire.command(MEMORY_WRITE, &[])?;
        self.wire.pixels(&frame.bytes(area))
    }

    /// Display on: the controller's memory is on the glass.
    fn light(&mut self) -> Result<()> {
        if !self.lit {
            self.wire.command(DISPLAY_ON, &[])?;
            self.wire.pause(Duration::from_millis(20));
            self.lit = true;
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn wire(&self) -> &W {
        &self.wire
    }
}

/// How the panel is wired, as the settings give it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wiring {
    pub spi_device: String,
    pub spi_speed_hz: u32,
    pub dc_gpio: u32,
    pub reset_gpio: u32,
    /// The GPIO chip's label, where it is not the SoC's own.
    pub gpio_chip_label: Option<String>,
}

/// The bus and the two lines.
pub struct Spi {
    spi: Spidev,
    dc: LineHandle,
    rst: LineHandle,
    chunk: usize,
    description: String,
}

impl Spi {
    pub fn open(wiring: &Wiring) -> Result<Self> {
        let mut spi = handover::open(&wiring.spi_device, || Spidev::open(&wiring.spi_device))
            .map_err(|e| {
                format!(
                    "{}: {e}{}",
                    wiring.spi_device,
                    if e.kind() == std::io::ErrorKind::NotFound {
                        "; the SPI bus is switched on in the boot configuration, which takes \
                     effect when the player has restarted"
                    } else {
                        ""
                    }
                )
            })?;
        spi.configure(
            &SpidevOptions::new()
                .bits_per_word(8)
                .max_speed_hz(wiring.spi_speed_hz)
                .mode(SpiModeFlags::SPI_MODE_0)
                .build(),
        )
        .map_err(|e| format!("{}: configure: {e}", wiring.spi_device))?;

        let mut chip = audiophonics_wire::open_chip(wiring.gpio_chip_label.as_deref())?;
        let dc = audiophonics_wire::output(&mut chip, wiring.dc_gpio, 0, "dc", CONSUMER)?;
        let rst = audiophonics_wire::output(&mut chip, wiring.reset_gpio, 1, "reset", CONSUMER)?;
        let chunk = audiophonics_wire::transfer_size();
        let description = format!(
            "{} at {} Hz, dc gpio {} and reset gpio {} on {} ({}), {} byte transfers",
            wiring.spi_device,
            wiring.spi_speed_hz,
            wiring.dc_gpio,
            wiring.reset_gpio,
            chip.name(),
            chip.label(),
            chunk
        );
        Ok(Self {
            spi,
            dc,
            rst,
            chunk,
            description,
        })
    }

    pub fn describe(&self) -> &str {
        &self.description
    }
}

fn gpio(what: &'static str) -> impl Fn(gpio_cdev::Error) -> String {
    move |e| format!("gpio {what}: {e}")
}

impl Wire for Spi {
    fn reset(&mut self) -> Result<()> {
        self.rst.set_value(0).map_err(gpio("reset low"))?;
        std::thread::sleep(Duration::from_millis(1));
        self.rst.set_value(1).map_err(gpio("reset high"))?;
        std::thread::sleep(Duration::from_millis(120));
        Ok(())
    }

    fn command(&mut self, command: u8, data: &[u8]) -> Result<()> {
        self.dc.set_value(0).map_err(gpio("dc"))?;
        self.spi
            .write_all(&[command])
            .map_err(|e| format!("spi command {command:#04x}: {e}"))?;
        if !data.is_empty() {
            self.dc.set_value(1).map_err(gpio("dc"))?;
            self.spi
                .write_all(data)
                .map_err(|e| format!("spi data for {command:#04x}: {e}"))?;
        }
        Ok(())
    }

    /// The controller's write pointer survives the chip-select gaps between
    /// transfers, so the pixels go out in pieces no larger than spidev's
    /// buffer.
    fn pixels(&mut self, bytes: &[u8]) -> Result<()> {
        self.dc.set_value(1).map_err(gpio("dc"))?;
        for piece in bytes.chunks(self.chunk) {
            self.spi
                .write_all(piece)
                .map_err(|e| format!("spi write: {e}"))?;
        }
        Ok(())
    }

    fn pause(&mut self, time: Duration) {
        std::thread::sleep(time);
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::canvas::{Canvas, WHITE};

    /// What went over the wire, for the tests: nothing is slept.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Sent {
        Reset,
        Command(u8, Vec<u8>),
        Pixels(usize),
        Pause(u64),
    }

    #[derive(Default)]
    pub struct Recorder {
        pub sent: Vec<Sent>,
        pub last_pixels: Vec<u8>,
        pub fail_at: Option<usize>,
    }

    impl Recorder {
        fn note(&mut self, sent: Sent) -> Result<()> {
            if self.fail_at == Some(self.sent.len()) {
                return Err("the bus went away".to_string());
            }
            self.sent.push(sent);
            Ok(())
        }
    }

    impl Wire for Recorder {
        fn reset(&mut self) -> Result<()> {
            self.note(Sent::Reset)
        }
        fn command(&mut self, command: u8, data: &[u8]) -> Result<()> {
            self.note(Sent::Command(command, data.to_vec()))
        }
        fn pixels(&mut self, bytes: &[u8]) -> Result<()> {
            self.last_pixels = bytes.to_vec();
            self.note(Sent::Pixels(bytes.len()))
        }
        fn pause(&mut self, time: Duration) {
            self.sent.push(Sent::Pause(time.as_millis() as u64));
        }
    }

    fn place(sent: &[Sent], wanted: &Sent) -> usize {
        sent.iter()
            .position(|s| s == wanted)
            .unwrap_or_else(|| panic!("{wanted:?} was not sent"))
    }

    #[test]
    fn the_controller_is_reset_set_up_cleared_and_only_then_lit() {
        let panel = Ili9341::start(Recorder::default()).unwrap();
        let sent = &panel.wire().sent;
        assert_eq!(sent[0], Sent::Reset);
        assert_eq!(sent[1], Sent::Command(SOFT_RESET, vec![]));
        assert_eq!(sent[2], Sent::Pause(5));
        // Sixteen bits a pixel, and the glass on its side in blue-green-red.
        let format = place(sent, &Sent::Command(PIXEL_FORMAT, vec![0x55]));
        let mode = place(sent, &Sent::Command(ADDRESS_MODE, vec![0xE8]));
        let woken = place(sent, &Sent::Command(SLEEP_OUT, vec![]));
        assert!(format < woken && mode < woken);
        assert_eq!(
            sent[woken + 1],
            Sent::Pause(120),
            "sleep out needs its time"
        );
        // The whole memory is written black before the display comes on.
        let cleared = place(sent, &Sent::Pixels(320 * 240 * 2));
        let lit = place(sent, &Sent::Command(DISPLAY_ON, vec![]));
        assert!(woken < cleared && cleared < lit);
        assert!(panel.wire().last_pixels.iter().all(|&b| b == 0));
        // Every command of the start is one the controller has, with data.
        assert_eq!(START.len(), 20);
        assert!(START.iter().all(|(_, data)| data.len() <= 15));
    }

    #[test]
    fn an_area_is_sent_as_its_window_and_its_pixels() {
        let mut panel = Ili9341::start(Recorder::default()).unwrap();
        let before = panel.wire().sent.len();
        let mut canvas = Canvas::new();
        canvas.set(300, 200, WHITE);
        let frame = Frame::from(&canvas);
        panel.show(&frame, Area::new(300, 200, 2, 3)).unwrap();
        assert_eq!(
            panel.wire().sent[before..],
            [
                // Columns 300 to 301 and rows 200 to 202, high byte first.
                Sent::Command(COLUMN_ADDRESS, vec![0x01, 0x2C, 0x01, 0x2D]),
                Sent::Command(PAGE_ADDRESS, vec![0x00, 0xC8, 0x00, 0xCA]),
                Sent::Command(MEMORY_WRITE, vec![]),
                Sent::Pixels(12),
            ]
        );
        assert_eq!(panel.wire().last_pixels[..4], [0xFF, 0xFF, 0x00, 0x00]);
    }

    #[test]
    fn what_is_not_an_area_of_the_glass_is_refused_before_anything_is_sent() {
        let mut panel = Ili9341::start(Recorder::default()).unwrap();
        let before = panel.wire().sent.len();
        let frame = Frame::black();
        for area in [
            Area::new(0, 0, 0, 5),
            Area::new(0, 0, 5, 0),
            Area::new(319, 0, 2, 1),
            Area::new(0, 239, 1, 2),
        ] {
            assert!(panel.show(&frame, area).is_err(), "{area:?}");
        }
        assert_eq!(panel.wire().sent.len(), before);
        panel.show(&frame, Area::new(319, 239, 1, 1)).unwrap();
    }

    #[test]
    fn a_bus_that_fails_is_an_error_and_not_a_panel() {
        for at in [0, 1, 5, 28, 29] {
            let wire = Recorder {
                fail_at: Some(at),
                ..Recorder::default()
            };
            let said = Ili9341::start(wire).err().unwrap();
            assert!(said.contains("went away"), "{said}");
        }
    }
}
