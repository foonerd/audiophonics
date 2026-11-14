//! Text on the glass.
//!
//! A line is drawn in DejaVu Sans, an outline face, at whatever size its
//! place on the screen asks for. A character DejaVu does not have, which is
//! most of Chinese, Japanese and Korean, is drawn from GNU Unifont, a
//! bitmap face sixteen pixels high that has the whole Basic Multilingual
//! Plane, brought to the line's size; so a line in two scripts stays one
//! line on one baseline. Without the outline face's file every character
//! comes from Unifont, and without either a character takes its room and
//! shows nothing: a missing file is a plainer screen, never a dead one.
//!
//! Both faces give a character as coverage, how much of each pixel the
//! glyph takes, and the canvas lays the line's colour over what is there
//! by that much, which is what makes an edge smooth over a cover.

use crate::canvas::{Canvas, Rgb};
use audiophonics_unifont::Unifont;
use std::collections::HashMap;
use std::path::Path;

/// The outline face's file, in the plugin's assets.
pub const OUTLINE_FILE: &str = "DejaVuSans.ttf";
/// The bitmap face's file, beside it.
pub const BITMAP_FILE: &str = "unifont.hex";

/// Unifont's glyphs are this high, and stand on a baseline two rows up.
const BITMAP_HEIGHT: u32 = 16;
const BITMAP_ASCENT: u32 = 14;
/// Shapes kept once made; a long session in many scripts starts over.
const KEPT: usize = 1024;
/// Samples a side taken of each pixel when a bitmap glyph is resized.
const SAMPLES: u32 = 4;

/// One character at one size, ready to be laid on a canvas.
struct Shape {
    /// How far the next character starts to the right.
    advance: i32,
    /// Where the coverage starts, right of the pen.
    left: i32,
    /// Where it starts, in rows above the baseline.
    top: i32,
    width: usize,
    /// Rows of `width`: how much of each pixel is the glyph's, 0 to 255.
    cover: Vec<u8>,
}

pub struct Faces {
    outline: Option<fontdue::Font>,
    bitmap: Unifont,
    shapes: HashMap<(char, u32), Shape>,
    problem: Option<String>,
}

impl Faces {
    /// The faces in a folder of assets. What is missing or unreadable is
    /// done without, and said once through [`Faces::take_problem`].
    pub fn open(assets: &Path) -> Self {
        let path = assets.join(OUTLINE_FILE);
        let (outline, problem) = match std::fs::read(&path) {
            Ok(bytes) => match fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                Ok(font) => (Some(font), None),
                Err(e) => (None, Some(format!("{}: {e}", path.display()))),
            },
            Err(e) => (None, Some(format!("{}: {e}", path.display()))),
        };
        Self {
            outline,
            bitmap: Unifont::new(assets.join(BITMAP_FILE)),
            shapes: HashMap::new(),
            problem,
        }
    }

    /// Why a face could not be used, each said once.
    pub fn take_problem(&mut self) -> Option<String> {
        self.problem.take().or_else(|| self.bitmap.take_problem())
    }

    fn shape(&mut self, ch: char, size: u32) -> &Shape {
        if self.shapes.len() >= KEPT && !self.shapes.contains_key(&(ch, size)) {
            self.shapes.clear();
        }
        let (outline, bitmap) = (&self.outline, &mut self.bitmap);
        self.shapes
            .entry((ch, size))
            .or_insert_with(|| make(outline.as_ref(), bitmap, ch, size))
    }

    /// The width of a line at a size, in pixels.
    pub fn width(&mut self, text: &str, size: u32) -> i32 {
        text.chars().map(|ch| self.shape(ch, size).advance).sum()
    }

    /// Draw a line with its pen starting at `x` and its baseline on row
    /// `baseline`. Returns where the pen ended.
    pub fn draw(
        &mut self,
        canvas: &mut Canvas,
        text: &str,
        x: i32,
        baseline: i32,
        size: u32,
        colour: Rgb,
    ) -> i32 {
        let mut pen = x;
        for ch in text.chars() {
            let shape = self.shape(ch, size);
            if shape.width > 0 {
                for (row, line) in shape.cover.chunks_exact(shape.width).enumerate() {
                    let y = baseline - shape.top + row as i32;
                    for (col, &cover) in line.iter().enumerate() {
                        if cover > 0 {
                            canvas.blend(pen + shape.left + col as i32, y, colour, cover);
                        }
                    }
                }
            }
            pen += shape.advance;
        }
        pen
    }
}

fn make(outline: Option<&fontdue::Font>, bitmap: &mut Unifont, ch: char, size: u32) -> Shape {
    if let Some(font) = outline.filter(|font| font.has_glyph(ch)) {
        let (metrics, cover) = font.rasterize(ch, size as f32);
        return Shape {
            advance: metrics.advance_width.round() as i32,
            left: metrics.xmin,
            top: metrics.ymin + metrics.height as i32,
            width: metrics.width,
            cover,
        };
    }
    match bitmap.glyph(ch) {
        Some(glyph) => resized(&glyph.rows, glyph.width, size),
        // Nothing can draw it: it takes a narrow character's room.
        None => Shape {
            advance: (size / 2) as i32,
            left: 0,
            top: 0,
            width: 0,
            cover: Vec::new(),
        },
    }
}

/// A bitmap glyph brought to a line's size. Each pixel of the result is
/// sampled sixteen times from the glyph, so at its own size the glyph comes
/// out as it is, and at any other its edges are as soft as the outline
/// face's.
fn resized(rows: &[u16; 16], width: u32, size: u32) -> Shape {
    let size = size.max(1);
    let out_w = ((width * size + BITMAP_HEIGHT / 2) / BITMAP_HEIGHT).max(1);
    let lit =
        |x: u32, y: u32| x < width && y < BITMAP_HEIGHT && rows[y as usize] & (0x8000 >> x) != 0;
    // A sample's place in the glyph: the centre of one of SAMPLES parts of
    // an output pixel, scaled by sixteen over the size.
    let at = |pixel: u32, part: u32, span: u32, from: u32| {
        (((pixel * SAMPLES + part) * 2 + 1) * from) / (span * SAMPLES * 2)
    };
    let mut cover = Vec::with_capacity((out_w * size) as usize);
    for y in 0..size {
        for x in 0..out_w {
            let mut hits = 0u32;
            for j in 0..SAMPLES {
                for i in 0..SAMPLES {
                    if lit(at(x, i, out_w, width), at(y, j, size, BITMAP_HEIGHT)) {
                        hits += 1;
                    }
                }
            }
            cover.push((hits * 255 / (SAMPLES * SAMPLES)) as u8);
        }
    }
    Shape {
        advance: out_w as i32,
        left: 0,
        top: ((BITMAP_ASCENT * size + BITMAP_HEIGHT / 2) / BITMAP_HEIGHT) as i32,
        width: out_w as usize,
        cover,
    }
}

/// A line as it is drawn: nothing that has no width, and every kind of
/// space a plain one.
pub fn plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{3000}' | '\u{00A0}' | '\t' | '\n' => {
                out.push(' ');
            }
            '\u{200B}'..='\u{200F}' | '\u{2060}' | '\u{FEFF}' => {}
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::canvas::{BLACK, WHITE};
    use std::path::PathBuf;

    /// The assets the plugin ships its faces in.
    pub fn assets() -> PathBuf {
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/dejavu"))
    }

    /// The rows and columns of a canvas that hold anything.
    fn inked(canvas: &Canvas) -> Option<(i32, i32, i32, i32)> {
        let mut found: Option<(i32, i32, i32, i32)> = None;
        for y in 0..240 {
            for x in 0..320 {
                if canvas.get(x, y) != BLACK {
                    let (l, t, r, b) = found.unwrap_or((x, y, x, y));
                    found = Some((l.min(x), t.min(y), r.max(x), b.max(y)));
                }
            }
        }
        found
    }

    #[test]
    fn the_outline_face_draws_on_its_baseline_at_the_size_asked() {
        let mut faces = Faces::open(&assets());
        assert_eq!(faces.take_problem(), None, "the outline face is shipped");
        let mut canvas = Canvas::new();
        let end = faces.draw(&mut canvas, "Hxg", 10, 100, 25, WHITE);
        let (left, top, right, bottom) = inked(&canvas).unwrap();
        assert!((10..=12).contains(&left), "starts at the pen: {left}");
        assert!(
            (80..=83).contains(&top),
            "a capital is about 18 rows: {top}"
        );
        assert!((103..=107).contains(&bottom), "the g hangs below: {bottom}");
        assert!(right < end, "the pen ends past the last glyph");
        assert_eq!(end - 10, faces.width("Hxg", 25));
        // Twice the size is about twice the line.
        let (small, large) = (faces.width("Brothers", 14), faces.width("Brothers", 28));
        assert!((large - 2 * small).abs() <= 4, "{small} against {large}");
    }

    #[test]
    fn an_edge_is_soft_and_the_colour_is_the_one_asked_for() {
        let mut faces = Faces::open(&assets());
        let mut canvas = Canvas::new();
        faces.draw(&mut canvas, "O", 10, 100, 40, [255, 0, 0]);
        let mut full = 0;
        let mut part = 0;
        for y in 0..240 {
            for x in 0..320 {
                let [r, g, b] = canvas.get(x, y);
                assert_eq!((g, b), (0, 0), "only red is laid on");
                match r {
                    0 => {}
                    255 => full += 1,
                    _ => part += 1,
                }
            }
        }
        assert!(full > 50 && part > 50, "{full} full, {part} soft");
    }

    #[test]
    fn what_the_outline_face_lacks_comes_from_the_bitmap_face_on_the_same_line() {
        let font = audiophonics_unifont::sample("mixed");
        let mut faces = Faces::open(&assets());
        faces.bitmap = Unifont::new(&font);
        assert!(faces.outline.as_ref().unwrap().has_glyph('A'));
        assert!(!faces.outline.as_ref().unwrap().has_glyph('中'));
        // At the bitmap's own size the glyph is as it is in the file.
        let mut canvas = Canvas::new();
        let end = faces.draw(&mut canvas, "中", 0, 14, 16, WHITE);
        assert_eq!(end, 16);
        assert_eq!(canvas.get(7, 0), WHITE, "0x0100: the eighth pixel of row 0");
        assert_eq!(canvas.get(6, 0), BLACK);
        assert_eq!(canvas.get(0, 3), WHITE, "0xFFFE: row 3 from the left edge");
        assert_eq!(canvas.get(15, 3), BLACK);
        // At another size it keeps its proportions and its baseline.
        let mut canvas = Canvas::new();
        let end = faces.draw(&mut canvas, "中", 0, 100, 24, WHITE);
        assert_eq!(end, 24);
        let (_, top, _, bottom) = inked(&canvas).unwrap();
        assert_eq!(
            top,
            100 - 21,
            "fourteen sixteenths of 24 above the baseline"
        );
        assert_eq!(
            bottom, 101,
            "its fifteenth row, the first below the baseline"
        );
        // A line in both scripts is one line.
        let mixed = faces.width("A中", 24);
        assert_eq!(mixed, faces.width("A", 24) + 24);
        std::fs::remove_file(&font).unwrap();
    }

    #[test]
    fn without_the_outline_face_everything_comes_from_the_bitmap_face() {
        let font = audiophonics_unifont::sample("alone");
        let dir = font.parent().unwrap().to_path_buf();
        let mut faces = Faces::open(&dir);
        faces.bitmap = Unifont::new(&font);
        assert!(faces.take_problem().unwrap().contains(OUTLINE_FILE));
        assert_eq!(faces.take_problem(), None, "said once");
        let mut canvas = Canvas::new();
        assert_eq!(faces.draw(&mut canvas, "A", 0, 14, 16, WHITE), 8);
        assert_eq!(canvas.get(3, 4), WHITE, "0x18: the fourth and fifth pixels");
        assert_eq!(canvas.get(4, 4), WHITE);
        assert_eq!(canvas.get(2, 4), BLACK);
        std::fs::remove_file(&font).unwrap();
    }

    #[test]
    fn a_character_no_face_has_takes_its_room_and_shows_nothing() {
        let mut faces = Faces::open(Path::new("/nonexistent"));
        let mut canvas = Canvas::new();
        assert_eq!(faces.draw(&mut canvas, "ab", 5, 50, 20, WHITE), 25);
        assert_eq!(canvas, Canvas::new());
        assert!(faces.take_problem().unwrap().contains("/nonexistent"));
        assert!(faces.take_problem().unwrap().contains("unifont.hex"));
        assert_eq!(faces.take_problem(), None);
    }

    #[test]
    fn a_line_is_cleared_of_what_has_no_width() {
        assert_eq!(plain("So\u{00A0}Far\tAway\n"), "So Far Away ");
        assert_eq!(plain("a\u{200B}b\u{FEFF}c\u{0007}"), "abc");
        assert_eq!(
            plain("\u{6771}\u{4EAC} \u{2014} \u{2019}x\u{2019}"),
            "東京 — ’x’"
        );
    }
}
