//! The mark shown while the player starts and when it leaves.
//!
//! The mark is a binary PGM (P5) in the plugin's assets: eight-bit grey, no
//! larger than the glass, light on dark. Its grey is how strongly white is
//! laid on, so a mark drawn with soft edges keeps them. A mark that cannot
//! be read is reported, and the screens that carry it show their words
//! alone: the panel never stays dark over a bad file.

use crate::canvas::{Canvas, HEIGHT, WHITE, WIDTH};
use std::path::Path;

/// The mark's file, in the plugin's assets.
pub const FILE: &str = "mark.pgm";

#[derive(Clone, PartialEq, Eq)]
pub struct Mark {
    pub width: u32,
    pub height: u32,
    /// Rows of `width`: how light each pixel is.
    luma: Vec<u8>,
}

impl std::fmt::Debug for Mark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Mark {}x{}", self.width, self.height)
    }
}

impl Mark {
    pub fn open(assets: &Path) -> Result<Mark, String> {
        let path = assets.join(FILE);
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Mark::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Read a binary PGM. Its header is "P5", the width, the height and the
    /// greatest grey, apart by white space, with comments from a `#` to the
    /// end of a line; one white-space byte, then the pixels.
    pub fn parse(bytes: &[u8]) -> Result<Mark, String> {
        let mut at = 0;
        let mut field = || -> Result<&[u8], String> {
            loop {
                while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
                    at += 1;
                }
                if bytes.get(at) == Some(&b'#') {
                    while bytes.get(at).is_some_and(|&b| b != b'\n') {
                        at += 1;
                    }
                } else {
                    break;
                }
            }
            let start = at;
            while bytes.get(at).is_some_and(|b| !b.is_ascii_whitespace()) {
                at += 1;
            }
            if start == at {
                return Err("the header ends too soon".to_string());
            }
            Ok(&bytes[start..at])
        };
        if field()? != b"P5" {
            return Err("not a binary PGM (P5)".to_string());
        }
        let mut number = |what: &str| -> Result<u32, String> {
            std::str::from_utf8(field()?)
                .ok()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| format!("the {what} is not a number"))
        };
        let width = number("width")?;
        let height = number("height")?;
        let greatest = number("greatest grey")?;
        if width == 0 || height == 0 || width > WIDTH || height > HEIGHT {
            return Err(format!(
                "{width}x{height}: a mark is at most {WIDTH}x{HEIGHT}"
            ));
        }
        if greatest == 0 || greatest > 255 {
            return Err(format!("greatest grey {greatest}: eight bits are read"));
        }
        // One white-space byte parts the header from the pixels.
        let pixels = bytes.get(at + 1..).unwrap_or_default();
        let wanted = (width * height) as usize;
        if pixels.len() < wanted {
            return Err(format!("{} pixels where {wanted} are needed", pixels.len()));
        }
        let luma = pixels[..wanted]
            .iter()
            .map(|&g| (u32::from(g).min(greatest) * 255 / greatest) as u8)
            .collect();
        Ok(Mark {
            width,
            height,
            luma,
        })
    }

    /// Draw the mark centred on column `centre` with its top on row `top`.
    pub fn draw(&self, canvas: &mut Canvas, centre: i32, top: i32) {
        let left = centre - self.width as i32 / 2;
        for (row, line) in self.luma.chunks_exact(self.width as usize).enumerate() {
            for (col, &luma) in line.iter().enumerate() {
                if luma > 0 {
                    canvas.blend(left + col as i32, top + row as i32, WHITE, luma);
                }
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::canvas::BLACK;

    /// A small mark for the tests: a light bar with a soft end.
    pub fn sample() -> Mark {
        Mark::parse(b"P5\n4 2\n255\n\xFF\xFF\x80\x00\xFF\xFF\x80\x00").unwrap()
    }

    #[test]
    fn a_pgm_is_read_with_its_comments_and_its_own_greatest_grey() {
        let mark = sample();
        assert_eq!((mark.width, mark.height), (4, 2));
        assert_eq!(mark.luma, [255, 255, 128, 0, 255, 255, 128, 0]);
        let commented = Mark::parse(b"P5 # a mark\n# made by hand\n2 1 15\n\x0F\x05").unwrap();
        assert_eq!(commented.luma, [255, 85]);
    }

    #[test]
    fn what_is_not_a_mark_is_refused_with_its_reason() {
        for (bytes, reason) in [
            (&b"P6\n2 1\n255\n\x00\x00"[..], "not a binary PGM"),
            (&b"P5\n2"[..], "ends too soon"),
            (&b"P5\nwide 1\n255\n\x00"[..], "width is not a number"),
            (&b"P5\n321 1\n255\n"[..], "at most 320x240"),
            (&b"P5\n0 1\n255\n"[..], "at most 320x240"),
            (&b"P5\n2 2\n65535\n\x00\x00\x00\x00"[..], "eight bits"),
            (&b"P5\n2 2\n255\n\x00\x00\x00"[..], "3 pixels where 4"),
            (&b""[..], "ends too soon"),
        ] {
            let said = Mark::parse(bytes).unwrap_err();
            assert!(said.contains(reason), "{said}");
        }
        assert!(Mark::open(Path::new("/nonexistent"))
            .unwrap_err()
            .contains("/nonexistent/mark.pgm"));
    }

    #[test]
    fn the_mark_is_centred_and_its_grey_is_how_white_it_is() {
        let mut canvas = Canvas::new();
        sample().draw(&mut canvas, 160, 100);
        assert_eq!(canvas.get(158, 100), WHITE);
        assert_eq!(canvas.get(159, 101), WHITE);
        assert_eq!(canvas.get(160, 100), [128, 128, 128]);
        assert_eq!(canvas.get(161, 100), BLACK);
        assert_eq!(canvas.get(157, 100), BLACK);
        assert_eq!(canvas.get(158, 102), BLACK);
    }
}
