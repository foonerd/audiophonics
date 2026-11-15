//! The two ways the panel is driven.
//!
//! As a framebuffer, the kernel's driver has the panel and this program
//! writes pictures into the kernel's copy; the panel is then also there
//! for the player's boot screens and for the progress of a system update.
//! Over SPI, this program has the panel to itself and sends it only the
//! rectangle that changed; nothing else can draw on it, and nothing does
//! before this program runs. Which of the two is a setting, and the boot
//! configuration follows it: the two cannot share the panel.

use crate::canvas::{Area, Frame};
use crate::config::{Config, Driver};
use crate::fbdev::FbDev;
use crate::ili9341::{Ili9341, Spi};

pub enum Panel {
    Framebuffer(FbDev),
    Spi(Box<Ili9341<Spi>>),
}

impl Panel {
    /// Open the panel the way the settings say. Returns it with a line
    /// describing how it was found.
    pub fn open(cfg: &Config) -> Result<(Self, String), String> {
        match cfg.driver {
            Driver::Framebuffer => {
                let fb = FbDev::open()?;
                let line = format!("framebuffer {}", fb.describe());
                Ok((Panel::Framebuffer(fb), line))
            }
            Driver::Spi => {
                let wire = Spi::open(&cfg.wiring)?;
                let line = wire.describe().to_string();
                Ok((Panel::Spi(Box::new(Ili9341::start(wire)?)), line))
            }
        }
    }

    /// True for a panel others can draw on too.
    pub fn shared(&self) -> bool {
        matches!(self, Panel::Framebuffer(_))
    }

    /// Send an area of a frame.
    pub fn show(&mut self, frame: &Frame, area: Area) -> Result<(), String> {
        match self {
            Panel::Framebuffer(fb) => fb.show(frame, area),
            Panel::Spi(panel) => panel.show(frame, area),
        }
    }
}
