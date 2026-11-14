//! Where a screen is composed, and what of it goes to the panel.
//!
//! A screen is put together in a [`Canvas`]: 320 x 240, eight bits to each
//! of red, green and blue, so a bar laid over a cover and a glyph's soft
//! edge are mixed exactly. The panel takes sixteen bits a pixel, five of
//! red, six of green and five of blue; a [`Frame`] is the canvas in that
//! form, which is what is sent, and two frames say which part of the glass
//! has to be sent again.
//!
//! The canvas is an embedded-graphics draw target, which is where the
//! primitives the icons are made of come from.

use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;

pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 240;
const PIXELS: usize = (WIDTH * HEIGHT) as usize;

/// Red, green and blue, eight bits each.
pub type Rgb = [u8; 3];

pub const BLACK: Rgb = [0, 0, 0];
pub const WHITE: Rgb = [255, 255, 255];

/// A rectangle of the glass: its left and top, its width and height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Area {
    pub const fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// The whole glass.
    pub const ALL: Area = Area::new(0, 0, WIDTH, HEIGHT);
}

#[derive(Clone, PartialEq, Eq)]
pub struct Canvas {
    px: Vec<Rgb>,
}

/// Not derived: a quarter of a megabyte in a test failure helps nobody.
impl std::fmt::Debug for Canvas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lit = self.px.iter().filter(|p| **p != BLACK).count();
        write!(f, "Canvas {WIDTH}x{HEIGHT}, {lit} pixels lit")
    }
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

impl Canvas {
    /// A black canvas.
    pub fn new() -> Self {
        Self {
            px: vec![BLACK; PIXELS],
        }
    }

    fn at(x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && (x as u32) < WIDTH && (y as u32) < HEIGHT)
            .then(|| y as usize * WIDTH as usize + x as usize)
    }

    /// The colour of a pixel; black beyond the edge.
    #[cfg(test)]
    pub fn get(&self, x: i32, y: i32) -> Rgb {
        Self::at(x, y).map_or(BLACK, |i| self.px[i])
    }

    /// Set one pixel. Beyond the edge nothing happens, which is what lets a
    /// line of text run off the glass.
    pub fn set(&mut self, x: i32, y: i32, colour: Rgb) {
        if let Some(i) = Self::at(x, y) {
            self.px[i] = colour;
        }
    }

    /// Lay a colour over a pixel as far as `cover` says: 0 leaves the pixel
    /// as it was, 255 replaces it. A glyph's edge is drawn this way.
    pub fn blend(&mut self, x: i32, y: i32, colour: Rgb, cover: u8) {
        if let Some(i) = Self::at(x, y) {
            self.px[i] = mix(self.px[i], colour, cover);
        }
    }

    pub fn fill(&mut self, colour: Rgb) {
        self.px.fill(colour);
    }

    /// The part of an area that is on the glass.
    fn clip(area: Area) -> Option<(usize, usize, usize, usize)> {
        if area.x >= WIDTH || area.y >= HEIGHT {
            return None;
        }
        let x1 = (area.x + area.w).min(WIDTH);
        let y1 = (area.y + area.h).min(HEIGHT);
        Some((area.x as usize, area.y as usize, x1 as usize, y1 as usize))
    }

    pub fn fill_area(&mut self, area: Area, colour: Rgb) {
        let Some((x0, y0, x1, y1)) = Self::clip(area) else {
            return;
        };
        for y in y0..y1 {
            let row = y * WIDTH as usize;
            self.px[row + x0..row + x1].fill(colour);
        }
    }

    /// Lay a colour over an area as far as `cover` says. A bar that the
    /// cover shows through is black laid over at seven tenths.
    pub fn veil(&mut self, area: Area, colour: Rgb, cover: u8) {
        let Some((x0, y0, x1, y1)) = Self::clip(area) else {
            return;
        };
        for y in y0..y1 {
            let row = y * WIDTH as usize;
            for p in &mut self.px[row + x0..row + x1] {
                *p = mix(*p, colour, cover);
            }
        }
    }

    /// Copy rows of pixels onto the canvas with their left top corner at
    /// (`x`, `y`); what falls beyond the edge is dropped. `pixels` holds
    /// rows of `width`.
    pub fn put(&mut self, x: i32, y: i32, width: u32, pixels: &[Rgb]) {
        if width == 0 {
            return;
        }
        for (row, line) in pixels.chunks_exact(width as usize).enumerate() {
            for (col, &colour) in line.iter().enumerate() {
                self.set(x + col as i32, y + row as i32, colour);
            }
        }
    }

    /// The canvas moved up by `rows`, with `below` following it from its row
    /// `from` on: one step of a page sliding over to the next, where the
    /// next page begins with the last `from` rows of this one.
    pub fn slid(&self, below: &Canvas, rows: u32, from: u32) -> Canvas {
        let from = from.min(HEIGHT);
        let rows = rows.min(HEIGHT - from) as usize;
        let w = WIDTH as usize;
        let mut px = Vec::with_capacity(PIXELS);
        px.extend_from_slice(&self.px[rows * w..]);
        px.extend_from_slice(&below.px[from as usize * w..(from as usize + rows) * w]);
        Canvas { px }
    }
}

/// `over` laid on `under` as far as `cover` says, rounded to nearest.
fn mix(under: Rgb, over: Rgb, cover: u8) -> Rgb {
    let a = u32::from(cover);
    let one = |u: u8, o: u8| ((u32::from(u) * (255 - a) + u32::from(o) * a + 127) / 255) as u8;
    [
        one(under[0], over[0]),
        one(under[1], over[1]),
        one(under[2], over[2]),
    ]
}

impl OriginDimensions for Canvas {
    fn size(&self) -> Size {
        Size::new(WIDTH, HEIGHT)
    }
}

impl DrawTarget for Canvas {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, colour) in pixels {
            self.set(point.x, point.y, [colour.r(), colour.g(), colour.b()]);
        }
        Ok(())
    }
}

/// A colour as the panel takes it: five bits of red, six of green, five of
/// blue, the low bits dropped.
pub fn rgb565(colour: Rgb) -> u16 {
    (u16::from(colour[0] >> 3) << 11) | (u16::from(colour[1] >> 2) << 5) | u16::from(colour[2] >> 3)
}

/// What the panel shows for a sixteen-bit colour, as eight bits a channel:
/// the high bits repeated into the low ones, so white is white.
pub fn rgb888(colour: u16) -> Rgb {
    let r = (colour >> 11) as u8 & 0x1F;
    let g = (colour >> 5) as u8 & 0x3F;
    let b = colour as u8 & 0x1F;
    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
    ]
}

/// A screen as the panel takes it: a sixteen-bit colour for each pixel.
#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    px: Vec<u16>,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lit = self.px.iter().filter(|p| **p != 0).count();
        write!(f, "Frame {WIDTH}x{HEIGHT}, {lit} pixels lit")
    }
}

impl From<&Canvas> for Frame {
    fn from(canvas: &Canvas) -> Self {
        Self {
            px: canvas.px.iter().map(|&c| rgb565(c)).collect(),
        }
    }
}

impl Frame {
    /// A black frame: what a panel shows that has been cleared.
    pub fn black() -> Self {
        Self {
            px: vec![0; PIXELS],
        }
    }

    pub fn get(&self, x: u32, y: u32) -> u16 {
        self.px[(y * WIDTH + x) as usize]
    }

    /// The pixels of an area, a row after another, each colour's high byte
    /// first: the order the panel's memory is written in.
    pub fn bytes(&self, area: Area) -> Vec<u8> {
        let mut out = Vec::with_capacity((area.w * area.h * 2) as usize);
        for y in area.y..area.y + area.h {
            let row = (y * WIDTH) as usize;
            for colour in &self.px[row + area.x as usize..row + (area.x + area.w) as usize] {
                out.extend_from_slice(&colour.to_be_bytes());
            }
        }
        out
    }

    /// The smallest rectangle outside which this frame is what `shown` is,
    /// or `None` when the two are the same: all of the glass that has to be
    /// sent to turn one into the other.
    pub fn changed(&self, shown: &Frame) -> Option<Area> {
        let w = WIDTH as usize;
        let differs = |y: usize| self.px[y * w..(y + 1) * w] != shown.px[y * w..(y + 1) * w];
        let top = (0..HEIGHT as usize).find(|&y| differs(y))?;
        let bottom = (top..HEIGHT as usize).rev().find(|&y| differs(y))?;
        let (mut left, mut right) = (w, 0);
        for y in top..=bottom {
            let (mine, theirs) = (&self.px[y * w..(y + 1) * w], &shown.px[y * w..(y + 1) * w]);
            if let Some(first) = (0..w).find(|&x| mine[x] != theirs[x]) {
                left = left.min(first);
                let last = (first..w).rev().find(|&x| mine[x] != theirs[x]);
                right = right.max(last.unwrap_or(first));
            }
        }
        Some(Area::new(
            left as u32,
            top as u32,
            (right - left + 1) as u32,
            (bottom - top + 1) as u32,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pixel_beyond_the_edge_is_dropped_and_reads_black() {
        let mut c = Canvas::new();
        c.set(-1, 0, WHITE);
        c.set(0, -1, WHITE);
        c.set(WIDTH as i32, 0, WHITE);
        c.set(0, HEIGHT as i32, WHITE);
        assert_eq!(c, Canvas::new());
        c.set(319, 239, WHITE);
        assert_eq!(c.get(319, 239), WHITE);
        assert_eq!(c.get(320, 239), BLACK);
    }

    #[test]
    fn a_colour_is_laid_over_as_far_as_its_cover_says() {
        let mut c = Canvas::new();
        c.fill([200, 100, 50]);
        c.blend(0, 0, WHITE, 0);
        assert_eq!(c.get(0, 0), [200, 100, 50]);
        c.blend(0, 0, WHITE, 255);
        assert_eq!(c.get(0, 0), WHITE);
        // Black at seven tenths leaves three tenths of what was there.
        c.veil(Area::new(10, 10, 2, 2), BLACK, 179);
        assert_eq!(c.get(10, 10), [60, 30, 15]);
        assert_eq!(c.get(11, 11), [60, 30, 15]);
        assert_eq!(c.get(12, 10), [200, 100, 50], "outside the area");
    }

    #[test]
    fn an_area_is_clipped_to_the_glass() {
        let mut c = Canvas::new();
        c.fill_area(Area::new(318, 238, 10, 10), WHITE);
        assert_eq!(c.get(318, 238), WHITE);
        assert_eq!(c.get(319, 239), WHITE);
        assert_eq!(c.get(317, 238), BLACK);
        c.fill_area(Area::new(400, 0, 5, 5), WHITE);
        c.veil(Area::new(0, 300, 5, 5), WHITE, 255);
    }

    #[test]
    fn rows_are_put_where_asked_and_cut_at_the_edge() {
        let mut c = Canvas::new();
        let red = [255, 0, 0];
        c.put(318, 0, 3, &[red; 6]);
        assert_eq!(c.get(318, 0), red);
        assert_eq!(c.get(319, 1), red);
        assert_eq!(c.get(0, 0), BLACK, "the third column fell off the edge");
        c.put(0, 0, 0, &[]);
    }

    #[test]
    fn a_page_slides_up_and_the_next_follows_it() {
        let mut above = Canvas::new();
        above.fill_area(Area::new(0, 0, WIDTH, 1), WHITE);
        above.fill_area(Area::new(0, 10, WIDTH, 1), [1, 2, 3]);
        above.fill_area(Area::new(0, 210, WIDTH, 30), [5, 5, 5]);
        // The next page starts with this one's last thirty rows.
        let mut below = Canvas::new();
        below.fill_area(Area::new(0, 0, WIDTH, 30), [5, 5, 5]);
        below.fill_area(Area::new(0, 30, WIDTH, 1), [9, 9, 9]);
        assert_eq!(above.slid(&below, 0, 30), above);
        let moved = above.slid(&below, 10, 30);
        assert_eq!(moved.get(0, 0), [1, 2, 3], "row 10 is now the top");
        assert_eq!(moved.get(0, 200), [5, 5, 5], "the last rows, ten higher");
        assert_eq!(
            moved.get(0, 230),
            [9, 9, 9],
            "the next page's row 30 follows"
        );
        assert_eq!(moved.get(0, 231), BLACK);
        // Moved all the way, it is the next page; it goes no further.
        assert_eq!(above.slid(&below, 210, 30), below);
        assert_eq!(above.slid(&below, 999, 30), below);
    }

    #[test]
    fn sixteen_bits_keep_the_high_bits_and_come_back_as_what_the_glass_shows() {
        assert_eq!(rgb565(WHITE), 0xFFFF);
        assert_eq!(rgb565(BLACK), 0);
        assert_eq!(rgb565([255, 0, 0]), 0xF800);
        assert_eq!(rgb565([0, 255, 0]), 0x07E0);
        assert_eq!(rgb565([0, 0, 255]), 0x001F);
        assert_eq!(rgb565([0xA7, 0x4A, 0x0C]), 0xA241);
        assert_eq!(rgb888(0xFFFF), WHITE);
        assert_eq!(rgb888(0), BLACK);
        // A colour the panel can show comes back as itself.
        let shown = rgb888(0xA241);
        assert_eq!(rgb565(shown), 0xA241);
    }

    #[test]
    fn a_frame_is_sent_high_byte_first() {
        let mut c = Canvas::new();
        c.set(1, 0, [255, 0, 0]);
        let f = Frame::from(&c);
        assert_eq!(f.get(1, 0), 0xF800);
        assert_eq!(f.bytes(Area::new(0, 0, 2, 1)), [0, 0, 0xF8, 0x00]);
        assert_eq!(f.bytes(Area::ALL).len(), 320 * 240 * 2);
    }

    #[test]
    fn two_frames_say_which_part_of_the_glass_changed() {
        let mut c = Canvas::new();
        let before = Frame::from(&c);
        assert_eq!(before.changed(&before), None);
        assert_eq!(before.changed(&Frame::black()), None);
        c.set(100, 50, WHITE);
        c.set(140, 60, WHITE);
        c.set(90, 55, WHITE);
        let after = Frame::from(&c);
        assert_eq!(after.changed(&before), Some(Area::new(90, 50, 51, 11)));
        // A change the panel cannot show is no change.
        let mut faint = Canvas::new();
        faint.set(0, 0, [7, 3, 7]);
        assert_eq!(Frame::from(&faint).changed(&before), None);
        let mut corner = Canvas::new();
        corner.set(319, 239, WHITE);
        assert_eq!(
            Frame::from(&corner).changed(&before),
            Some(Area::new(319, 239, 1, 1))
        );
    }
}
