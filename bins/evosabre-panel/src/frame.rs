//! The 256x64 frame the SSD1322 shows: sixteen greys, two pixels per byte,
//! the left pixel in the high nibble. This is exactly the controller's RAM
//! layout for a 256-column panel, so a frame is written out unchanged.
//!
//! It is an embedded-graphics draw target, which is where fonts, primitives
//! and layout come from.

use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;

pub const WIDTH: u32 = 256;
pub const HEIGHT: u32 = 64;
pub const BYTES: usize = (WIDTH as usize / 2) * HEIGHT as usize;

#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    bytes: Vec<u8>,
    /// Added to every coordinate: the pixel orbit moves the whole picture
    /// without the layout knowing. Pixels pushed off the edge are dropped.
    origin: (u32, u32),
}

/// Not derived: 8192 bytes in a test failure message help nobody.
impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lit = self
            .bytes
            .iter()
            .map(|b| usize::from(b >> 4 != 0) + usize::from(b & 0x0F != 0))
            .sum::<usize>();
        write!(f, "Frame {WIDTH}x{HEIGHT}, {lit} pixels lit")
    }
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

impl Frame {
    pub fn new() -> Self {
        Self {
            bytes: vec![0; BYTES],
            origin: (0, 0),
        }
    }

    pub fn set_origin(&mut self, dx: u32, dy: u32) {
        self.origin = (dx, dy);
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Set one pixel to a grey level 0-15. Off-frame coordinates are ignored,
    /// which is what lets text run off the right edge rather than panic.
    pub fn set(&mut self, x: u32, y: u32, luma: u8) {
        let (x, y) = (x + self.origin.0, y + self.origin.1);
        if x >= WIDTH || y >= HEIGHT {
            return;
        }
        let i = (x / 2 + y * (WIDTH / 2)) as usize;
        let b = &mut self.bytes[i];
        *b = if x & 1 == 0 {
            (*b & 0x0F) | (luma << 4)
        } else {
            (*b & 0xF0) | (luma & 0x0F)
        };
    }

    /// One pixel's grey level, 0-15.
    pub fn get(&self, x: u32, y: u32) -> u8 {
        let b = self.bytes[(x / 2 + y * (WIDTH / 2)) as usize];
        if x & 1 == 0 {
            b >> 4
        } else {
            b & 0x0F
        }
    }

    /// True if any pixel is lit.
    #[cfg(test)]
    pub fn is_lit(&self) -> bool {
        self.bytes.iter().any(|&b| b != 0)
    }
}

impl OriginDimensions for Frame {
    fn size(&self) -> Size {
        Size::new(WIDTH, HEIGHT)
    }
}

impl DrawTarget for Frame {
    type Color = Gray4;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(p, c) in pixels {
            if p.x >= 0 && p.y >= 0 {
                self.set(p.x as u32, p.y as u32, c.luma());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

    #[test]
    fn two_pixels_share_a_byte_left_in_the_high_nibble() {
        let mut f = Frame::new();
        f.set(0, 0, 0xF);
        assert_eq!(f.bytes()[0], 0xF0);
        f.set(1, 0, 0x3);
        assert_eq!(f.bytes()[0], 0xF3);
        f.set(255, 63, 0x9);
        assert_eq!(f.bytes()[BYTES - 1], 0x09);
        assert_eq!(f.get(1, 0), 3);
        assert_eq!(f.get(255, 63), 9);
    }

    #[test]
    fn rows_are_128_bytes_apart() {
        let mut f = Frame::new();
        f.set(0, 1, 0xF);
        assert_eq!(f.bytes()[128], 0xF0);
    }

    #[test]
    fn off_frame_pixels_are_ignored_not_panicked() {
        let mut f = Frame::new();
        f.set(256, 0, 0xF);
        f.set(0, 64, 0xF);
        assert!(!f.is_lit());
    }

    #[test]
    fn the_origin_moves_everything_and_drops_what_falls_off() {
        let mut f = Frame::new();
        f.set_origin(1, 1);
        f.set(0, 0, 0xF);
        assert_eq!(f.get(1, 1), 15);
        assert_eq!(f.get(0, 0), 0);
        f.set(255, 63, 0xF);
        assert_eq!(f.get(255, 63), 0, "pushed off the edge, not wrapped");
    }

    #[test]
    fn embedded_graphics_draws_into_it() {
        let mut f = Frame::new();
        Rectangle::new(Point::new(2, 2), Size::new(3, 2))
            .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
            .draw(&mut f)
            .unwrap();
        assert_eq!(f.get(2, 2), 15);
        assert_eq!(f.get(4, 3), 15);
        assert_eq!(f.get(5, 3), 0);
        assert_eq!(f.get(2, 4), 0);
    }
}
