//! The cover of what plays, made into the background of the screen.
//!
//! A cover arrives as a file of any size and shape: JPEG, PNG, GIF, WebP or
//! BMP. It becomes the 320 x 240 picture the first page is drawn over: the
//! cover brought to the height of the glass and centred, and, where it does
//! not reach the sides, itself again, stretched over the whole glass and
//! blurred until only its colours are left.
//!
//! The arithmetic is in whole numbers throughout, so the same cover makes
//! the same picture on the ARM a player runs and on the machine that checks
//! the screens. The panel has sixteen bits a pixel; a soft gradient cut
//! down to that shows as bands, so the picture is dithered onto the
//! panel's colours here, once, and everything drawn over it afterwards
//! stays as it is drawn.

use crate::canvas::{Rgb, BLACK, HEIGHT, WIDTH};

/// A cover is not decoded beyond this many pixels a side...
const MAX_SIDE: u32 = 4096;
/// ...or this much memory, whatever its file claims.
const MAX_ALLOC: u64 = 96 * 1024 * 1024;

/// The cover is made this small before it is blurred...
const BLUR_WIDTH: u32 = 32;
const BLUR_HEIGHT: u32 = 24;
/// ...by this much, this many times.
const BLUR_RADIUS: usize = 2;
const BLUR_PASSES: usize = 3;

/// A picture: rows of `width` pixels.
#[derive(Clone, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub px: Vec<Rgb>,
}

impl std::fmt::Debug for Picture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Picture {}x{}", self.width, self.height)
    }
}

impl Picture {
    /// A picture of one colour.
    pub fn filled(width: u32, height: u32, colour: Rgb) -> Self {
        Self {
            width,
            height,
            px: vec![colour; (width * height) as usize],
        }
    }

    /// Decode a cover's file. What is see-through in it is laid on black;
    /// of an animation, the first picture is taken.
    pub fn decode(file: &[u8]) -> Result<Picture, String> {
        let mut reader = image::ImageReader::new(std::io::Cursor::new(file))
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_SIDE);
        limits.max_image_height = Some(MAX_SIDE);
        limits.max_alloc = Some(MAX_ALLOC);
        reader.limits(limits);
        let decoded = reader.decode().map_err(|e| e.to_string())?.into_rgba8();
        let (width, height) = decoded.dimensions();
        if width == 0 || height == 0 {
            return Err("a picture with no pixels".to_string());
        }
        let px = decoded
            .pixels()
            .map(|p| {
                let a = u32::from(p.0[3]);
                let on_black = |c: u8| ((u32::from(c) * a + 127) / 255) as u8;
                [on_black(p.0[0]), on_black(p.0[1]), on_black(p.0[2])]
            })
            .collect();
        Ok(Picture { width, height, px })
    }

    fn get(&self, x: u32, y: u32) -> Rgb {
        self.px[(y * self.width + x) as usize]
    }

    /// The picture at another size. Made smaller, each pixel is the mean of
    /// what it covers; made larger, it is drawn between its neighbours.
    pub fn resized(&self, width: u32, height: u32) -> Picture {
        let (width, height) = (width.max(1), height.max(1));
        let across = Taps::new(self.width, width);
        let down = Taps::new(self.height, height);
        // Across first, into rows of the new width and the old height.
        let mut wide = Vec::with_capacity((width * self.height) as usize);
        for y in 0..self.height {
            for x in 0..width {
                wide.push(across.mix(x, |i| self.get(i, y)));
            }
        }
        let mut px = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            for x in 0..width {
                px.push(down.mix(y, |i| wide[(i * width + x) as usize]));
            }
        }
        Picture { width, height, px }
    }

    /// The picture with every pixel the mean of those around it, as far as
    /// `radius` to each side, `passes` times: three passes are close to a
    /// true soft blur. The edge pixels stand in for what lies beyond them.
    fn blurred(&self, radius: usize, passes: usize) -> Picture {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut px = self.px.clone();
        let mut line = Vec::new();
        for _ in 0..passes {
            for y in 0..h {
                line.clear();
                line.extend_from_slice(&px[y * w..(y + 1) * w]);
                for x in 0..w {
                    px[y * w + x] = mean(&line, x, radius);
                }
            }
            for x in 0..w {
                line.clear();
                line.extend((0..h).map(|y| px[y * w + x]));
                for y in 0..h {
                    px[y * w + x] = mean(&line, y, radius);
                }
            }
        }
        Picture {
            width: self.width,
            height: self.height,
            px,
        }
    }
}

/// The mean of a line's pixels from `radius` before `at` to `radius` after.
fn mean(line: &[Rgb], at: usize, radius: usize) -> Rgb {
    let last = line.len() - 1;
    let mut sum = [0u32; 3];
    for step in 0..=2 * radius {
        let i = (at + step).saturating_sub(radius).min(last);
        for (total, &part) in sum.iter_mut().zip(&line[i]) {
            *total += u32::from(part);
        }
    }
    let n = (2 * radius + 1) as u32;
    [
        ((sum[0] + n / 2) / n) as u8,
        ((sum[1] + n / 2) / n) as u8,
        ((sum[2] + n / 2) / n) as u8,
    ]
}

/// How one line of `from` pixels becomes one of `to`: for each new pixel,
/// the old pixels it is made of and how much of each.
struct Taps {
    /// For each new pixel: the first old pixel, and where its weights are.
    spans: Vec<(u32, std::ops::Range<usize>)>,
    weights: Vec<u32>,
    /// What a pixel's weights add up to.
    total: u32,
    last: u32,
}

impl Taps {
    fn new(from: u32, to: u32) -> Self {
        let mut spans = Vec::with_capacity(to as usize);
        let mut weights = Vec::new();
        let total;
        if to < from {
            // Smaller: new pixel `at` covers old pixels from at*from/to to
            // (at+1)*from/to. Counted in units of 1/to of an old pixel the
            // overlaps are whole numbers and add up to `from`.
            total = from;
            for at in 0..to {
                let (start, end) = (at * from, (at + 1) * from);
                let first = start / to;
                let begun = weights.len();
                let mut i = first;
                while i * to < end {
                    weights.push(end.min((i + 1) * to) - start.max(i * to));
                    i += 1;
                }
                spans.push((first, begun..weights.len()));
            }
        } else {
            // Larger, or the same: the centre of new pixel `at` lies
            // between two old pixels' centres; counted in units of 1/(2 to).
            total = 2 * to;
            for at in 0..to {
                let centre = i64::from((2 * at + 1) * from) - i64::from(to);
                let centre = centre.clamp(0, i64::from(2 * to * (from - 1))) as u32;
                let (first, part) = (centre / total, centre % total);
                let begun = weights.len();
                weights.push(total - part);
                weights.push(part);
                spans.push((first, begun..weights.len()));
            }
        }
        Self {
            spans,
            weights,
            total,
            last: from - 1,
        }
    }

    fn mix(&self, at: u32, old: impl Fn(u32) -> Rgb) -> Rgb {
        let (first, range) = &self.spans[at as usize];
        let mut sum = [0u32; 3];
        for (step, &weight) in self.weights[range.clone()].iter().enumerate() {
            if weight == 0 {
                continue;
            }
            let pixel = old((first + step as u32).min(self.last));
            for (total, &part) in sum.iter_mut().zip(&pixel) {
                *total += u32::from(part) * weight;
            }
        }
        let half = self.total / 2;
        [
            ((sum[0] + half) / self.total) as u8,
            ((sum[1] + half) / self.total) as u8,
            ((sum[2] + half) / self.total) as u8,
        ]
    }
}

/// How far a pixel is pushed towards the next colour the panel has, by
/// where it is: an ordered dither, four by four, in sixteenths.
const DITHER: [[u32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// One channel brought onto the levels the panel has for it, `bits` of
/// them, as the eight-bit value the panel shows: the level below, or the
/// one above when the value lies further towards it than `push` sixteenths.
fn level(value: u8, bits: u32, push: u32) -> u8 {
    let drop = 8 - bits;
    let top = (1u32 << bits) - 1;
    // What the panel shows for a level: its bits, repeated into the low ones.
    let shown = |l: u32| (l << drop) | (l >> (bits - drop));
    let value = u32::from(value);
    let mut below = value >> drop;
    if shown(below) > value {
        below -= 1;
    }
    if below >= top {
        return shown(top) as u8;
    }
    let (lo, hi) = (shown(below), shown(below + 1));
    if (value - lo) * 16 > push * (hi - lo) {
        hi as u8
    } else {
        lo as u8
    }
}

/// A colour brought onto the panel's sixteen bits for a pixel at (x, y),
/// and given back as the eight bits a channel the panel shows for it. A
/// colour the panel has is left as it is.
fn dithered(colour: Rgb, x: u32, y: u32) -> Rgb {
    let push = DITHER[(y & 3) as usize][(x & 3) as usize];
    [
        level(colour[0], 5, push),
        level(colour[1], 6, push),
        level(colour[2], 5, push),
    ]
}

/// The background a cover makes: 320 x 240 pixels, a row after another.
pub fn backdrop(cover: &Picture) -> Vec<Rgb> {
    // The colours of the cover, over the whole glass.
    let soft = cover
        .resized(BLUR_WIDTH, BLUR_HEIGHT)
        .blurred(BLUR_RADIUS, BLUR_PASSES)
        .resized(WIDTH, HEIGHT);
    // The cover itself at the height of the glass. One wider than the glass
    // keeps its middle.
    let width = (u64::from(cover.width) * u64::from(HEIGHT) / u64::from(cover.height)).max(1);
    let fitted = cover.resized(width.min(u64::from(MAX_SIDE)) as u32, HEIGHT);
    let mut px = Vec::with_capacity((WIDTH * HEIGHT) as usize);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            // Where this pixel of the glass is in the fitted cover.
            let at = i64::from(x) + (i64::from(fitted.width) - i64::from(WIDTH)) / 2;
            let colour = if (0..i64::from(fitted.width)).contains(&at) {
                fitted.get(at as u32, y)
            } else {
                soft.get(x, y)
            };
            px.push(dithered(colour, x, y));
        }
    }
    px
}

/// A cover made here, so the screens drawn for review need no file: four
/// quarters of colour with a pale disc in the middle.
pub fn sample(width: u32, height: u32) -> Picture {
    let mut px = Vec::with_capacity((width * height) as usize);
    let (half_w, half_h) = (i64::from(width / 2), i64::from(height / 2));
    let radius = half_w.min(half_h) / 2;
    for y in 0..height {
        for x in 0..width {
            let (dx, dy) = (i64::from(x) - half_w, i64::from(y) - half_h);
            px.push(if dx * dx + dy * dy < radius * radius {
                [240, 232, 208]
            } else {
                match (x < width / 2, y < height / 2) {
                    (true, true) => [196, 62, 44],
                    (false, true) => [36, 96, 150],
                    (true, false) => [232, 168, 40],
                    (false, false) => [40, 120, 96],
                }
            });
        }
    }
    Picture { width, height, px }
}

/// The background where there is no cover: black.
pub fn none() -> Vec<Rgb> {
    vec![BLACK; (WIDTH * HEIGHT) as usize]
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::canvas::{rgb565, rgb888, WHITE};

    fn encoded(picture: &Picture, format: image::ImageFormat) -> Vec<u8> {
        let raw: Vec<u8> = picture.px.iter().flatten().copied().collect();
        let buffer = image::RgbImage::from_raw(picture.width, picture.height, raw).unwrap();
        let mut file = std::io::Cursor::new(Vec::new());
        buffer.write_to(&mut file, format).unwrap();
        file.into_inner()
    }

    #[test]
    fn a_png_is_decoded_as_it_is_and_what_is_see_through_goes_to_black() {
        let cover = sample(64, 64);
        let back = Picture::decode(&encoded(&cover, image::ImageFormat::Png)).unwrap();
        assert_eq!(back, cover);

        let mut see_through = image::RgbaImage::new(2, 1);
        see_through.put_pixel(0, 0, image::Rgba([200, 100, 50, 255]));
        see_through.put_pixel(1, 0, image::Rgba([200, 100, 50, 128]));
        let mut file = std::io::Cursor::new(Vec::new());
        see_through
            .write_to(&mut file, image::ImageFormat::Png)
            .unwrap();
        let back = Picture::decode(&file.into_inner()).unwrap();
        assert_eq!(back.px, [[200, 100, 50], [100, 50, 25]]);
    }

    #[test]
    fn the_other_kinds_of_file_a_cover_comes_as_are_decoded() {
        let cover = sample(48, 48);
        for format in [
            image::ImageFormat::Jpeg,
            image::ImageFormat::Gif,
            image::ImageFormat::Bmp,
            image::ImageFormat::WebP,
        ] {
            let back = Picture::decode(&encoded(&cover, format)).unwrap();
            assert_eq!((back.width, back.height), (48, 48), "{format:?}");
            let corner = back.get(4, 4);
            assert!(
                corner[0] > 150 && corner[1] < 110 && corner[2] < 100,
                "{format:?}: the red quarter is {corner:?}"
            );
        }
    }

    #[test]
    fn what_is_not_a_picture_is_refused_and_so_is_one_too_large() {
        assert!(Picture::decode(b"").is_err());
        assert!(Picture::decode(b"<html>Not found</html>").is_err());
        let cut = encoded(&sample(64, 64), image::ImageFormat::Png);
        assert!(Picture::decode(&cut[..cut.len() / 2]).is_err());
        let wide = encoded(
            &Picture::filled(MAX_SIDE + 1, 1, WHITE),
            image::ImageFormat::Png,
        );
        assert!(Picture::decode(&wide).is_err());
    }

    #[test]
    fn made_smaller_a_pixel_is_the_mean_of_what_it_covers() {
        let mut p = Picture::filled(4, 2, BLACK);
        p.px[0] = [100, 0, 0];
        p.px[1] = [200, 0, 0];
        p.px[4] = [0, 40, 0];
        p.px[5] = [0, 80, 0];
        let small = p.resized(2, 1);
        assert_eq!(small.px, [[75, 30, 0], BLACK]);
        // Three into two: the middle pixel is shared.
        let mut p = Picture::filled(3, 1, BLACK);
        p.px[1] = [90, 90, 90];
        assert_eq!(p.resized(2, 1).px, [[30, 30, 30], [30, 30, 30]]);
        // The same size is the same picture.
        let cover = sample(16, 16);
        assert_eq!(cover.resized(16, 16), cover);
    }

    #[test]
    fn made_larger_a_pixel_lies_between_its_neighbours() {
        let p = Picture {
            width: 2,
            height: 1,
            px: vec![BLACK, [200, 200, 200]],
        };
        let large = p.resized(4, 1);
        assert_eq!(
            large.px,
            [BLACK, [50, 50, 50], [150, 150, 150], [200, 200, 200]]
        );
        let one = Picture::filled(1, 1, [9, 8, 7]).resized(5, 3);
        assert!(one.px.iter().all(|&c| c == [9, 8, 7]));
    }

    #[test]
    fn a_blur_spreads_a_point_and_keeps_a_flat_colour() {
        let flat = Picture::filled(8, 6, [10, 200, 30]);
        assert_eq!(flat.blurred(2, 3), flat);
        let mut point = Picture::filled(9, 9, BLACK);
        point.px[4 * 9 + 4] = WHITE;
        let soft = point.blurred(1, 1);
        assert_eq!(soft.get(4, 4), [28, 28, 28], "a ninth of white");
        assert_eq!(soft.get(3, 3), [28, 28, 28]);
        assert_eq!(soft.get(6, 4), BLACK);
    }

    #[test]
    fn a_square_cover_stands_in_the_middle_over_its_own_colours() {
        let px = backdrop(&sample(600, 600));
        assert_eq!(px.len(), 320 * 240);
        let at = |x: u32, y: u32| px[(y * WIDTH + x) as usize];
        // The cover is 240 wide, from column 40 to 279: its quarters...
        let red = at(60, 20);
        assert!(red[0] > 180 && red[2] < 60, "{red:?}");
        let blue = at(260, 20);
        assert!(blue[2] > 130 && blue[0] < 60, "{blue:?}");
        // ...and its disc in the middle.
        let disc = at(160, 120);
        assert!(disc[0] > 225 && disc[2] > 195, "{disc:?}");
        // Beside it, the cover's colours with nothing sharp in them: the
        // left edge is between the red above and the yellow below.
        let side = at(10, 120);
        assert!(side[0] > 150 && side[2] < 90, "{side:?}");
        let (up, down) = (at(10, 100), at(10, 140));
        assert!(up[1].abs_diff(down[1]) < 60, "soft: {up:?} {down:?}");
        // Every pixel is a colour the panel has.
        assert!(px.iter().all(|&c| rgb888(rgb565(c)) == c));
    }

    #[test]
    fn a_wide_cover_keeps_its_middle_and_a_tall_one_its_height() {
        let mut wide = Picture::filled(900, 300, [0, 0, 250]);
        for y in 0..300 {
            for x in 400..500 {
                wide.px[(y * 900 + x) as usize] = [250, 0, 0];
            }
        }
        let px = backdrop(&wide);
        let at = |x: u32, y: u32| px[(y * WIDTH + x) as usize];
        assert!(at(160, 120)[0] > 240, "the middle of the cover");
        assert!(at(5, 120)[2] > 240, "its blue, not a blur");
        let tall = backdrop(&Picture::filled(100, 400, [0, 250, 0]));
        assert!(tall.iter().all(|&c| c[1] > 240 && c[0] < 8));
        assert!(none().iter().all(|&c| c == BLACK));
    }

    #[test]
    fn a_gradient_is_dithered_and_a_colour_the_panel_has_is_left_alone() {
        let exact = rgb888(0xA241);
        for (x, y) in [(0, 0), (1, 2), (3, 3)] {
            assert_eq!(dithered(exact, x, y), exact);
        }
        // Halfway between two of the panel's reds: half the pixels each.
        let low = rgb888(0x8000)[0];
        let between = [low + 4, 0, 0];
        let mut up = 0;
        for y in 0..4 {
            for x in 0..4 {
                if dithered(between, x, y)[0] > low {
                    up += 1;
                }
            }
        }
        assert_eq!(up, 8);
        assert_eq!(dithered(WHITE, 3, 0), WHITE);
    }
}
