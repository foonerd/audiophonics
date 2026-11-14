//! GNU Unifont, read from its `.hex` file, for text a display's own faces
//! cannot draw: Chinese, Japanese, Korean, Cyrillic, Greek and the rest of
//! the Basic Multilingual Plane.
//!
//! A `.hex` file is one glyph a line: the code point in hexadecimal, a
//! colon, then sixteen rows of pixels, two digits a row for a glyph eight
//! pixels wide and four for one sixteen wide. The file is several
//! megabytes and most tracks need none of it, so nothing is read until a
//! line calls for it; then the file is walked once for where each glyph
//! starts, and a glyph is read when it is first drawn.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

pub const HEIGHT: u32 = 16;
/// What a character without a glyph takes up: the width of a narrow one.
pub const MISSING: u32 = 8;

/// The highest code point indexed: the Basic Multilingual Plane.
const LAST: usize = 0xFFFF;
const ABSENT: u32 = u32::MAX;
/// Set in an index entry whose glyph is sixteen pixels wide.
const WIDE: u32 = 1 << 31;
/// Glyphs kept once read; a long session in many scripts starts over.
const KEPT: usize = 2048;

/// One glyph: sixteen rows, the leftmost pixel in the highest bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    pub width: u32,
    pub rows: [u16; 16],
}

enum Source {
    Unopened,
    Open { file: File, index: Vec<u32> },
    Unusable,
}

pub struct Unifont {
    path: PathBuf,
    source: Source,
    glyphs: HashMap<char, Option<Glyph>>,
    problem: Option<String>,
}

impl Unifont {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            source: Source::Unopened,
            glyphs: HashMap::new(),
            problem: None,
        }
    }

    /// True when the file is there and readable. The first call opens it.
    pub fn usable(&mut self) -> bool {
        if matches!(self.source, Source::Unopened) {
            self.source = match open(&self.path) {
                Ok((file, index)) => Source::Open { file, index },
                Err(e) => {
                    self.problem = Some(format!("{}: {e}", self.path.display()));
                    Source::Unusable
                }
            };
        }
        matches!(self.source, Source::Open { .. })
    }

    /// Why the file could not be used, said once.
    pub fn take_problem(&mut self) -> Option<String> {
        self.problem.take()
    }

    pub fn glyph(&mut self, ch: char) -> Option<Glyph> {
        if let Some(known) = self.glyphs.get(&ch) {
            return *known;
        }
        if !self.usable() {
            return None;
        }
        let Source::Open { file, index } = &self.source else {
            return None;
        };
        let glyph = index
            .get(ch as usize)
            .copied()
            .filter(|&entry| entry != ABSENT)
            .and_then(|entry| read(file, entry));
        if self.glyphs.len() >= KEPT {
            self.glyphs.clear();
        }
        self.glyphs.insert(ch, glyph);
        glyph
    }

    /// The width of a line, in pixels.
    pub fn width(&mut self, s: &str) -> u32 {
        s.chars()
            .map(|ch| self.glyph(ch).map_or(MISSING, |g| g.width))
            .sum()
    }
}

/// Open the file and note where each glyph's rows start.
fn open(path: &PathBuf) -> Result<(File, Vec<u32>), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut index = vec![ABSENT; LAST + 1];
    let mut reader = BufReader::new(&file);
    let mut line = Vec::with_capacity(80);
    let mut at: u64 = 0;
    let mut found = 0usize;
    loop {
        line.clear();
        let n = reader
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        if let Some(colon) = line.iter().position(|&b| b == b':') {
            let digits = line[colon + 1..]
                .iter()
                .take_while(|b| b.is_ascii_hexdigit())
                .count();
            let code = std::str::from_utf8(&line[..colon])
                .ok()
                .and_then(|s| usize::from_str_radix(s.trim(), 16).ok());
            let start = at + colon as u64 + 1;
            if let (Some(code), true) = (code, start < u64::from(WIDE)) {
                if code <= LAST && (digits == 32 || digits == 64) {
                    index[code] = start as u32 | if digits == 64 { WIDE } else { 0 };
                    found += 1;
                }
            }
        }
        at += n as u64;
    }
    if found == 0 {
        return Err("no glyphs in it; not a Unifont .hex file".to_string());
    }
    Ok((file, index))
}

fn read(file: &File, entry: u32) -> Option<Glyph> {
    let wide = entry & WIDE != 0;
    let mut text = [0u8; 64];
    let text = &mut text[..if wide { 64 } else { 32 }];
    file.read_exact_at(text, u64::from(entry & !WIDE)).ok()?;
    let text = std::str::from_utf8(text).ok()?;
    let per_row = if wide { 4 } else { 2 };
    let mut rows = [0u16; 16];
    for (i, row) in rows.iter_mut().enumerate() {
        let bits = u16::from_str_radix(&text[i * per_row..(i + 1) * per_row], 16).ok()?;
        *row = if wide { bits } else { bits << 8 };
    }
    Some(Glyph {
        width: if wide { 16 } else { 8 },
        rows,
    })
}

/// A small file in Unifont's format, written to the temporary directory for
/// a test: a narrow A, a wide 中, a line that is not a glyph, and a glyph
/// beyond the plane this reads. The caller removes it.
#[cfg(any(test, feature = "sample"))]
pub fn sample(tag: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("unifont-sample-{tag}-{}.hex", std::process::id()));
    let narrow = "0000000018242442427E424242420000";
    let wide = "010001000100FFFE8102810281028102FFFE8102010001000100010001000000";
    std::fs::write(
        &path,
        format!("0041:{narrow}\n# not a glyph\n4E2D:{wide}\n01F600:{wide}\n"),
    )
    .expect("the temporary directory takes a small file");
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_and_wide_glyphs_are_read_with_the_left_pixel_in_the_high_bit() {
        let path = sample("glyphs");
        let mut font = Unifont::new(&path);
        let a = font.glyph('A').unwrap();
        assert_eq!(a.width, 8);
        assert_eq!(a.rows[4], 0x1800, "0x18 in the left byte");
        assert_eq!(a.rows[9], 0x7E00);
        let zhong = font.glyph('中').unwrap();
        assert_eq!(zhong.width, 16);
        assert_eq!(zhong.rows[3], 0xFFFE);
        assert_eq!(zhong.rows[0], 0x0100);
        assert_eq!(font.glyph('A'), Some(a), "kept once read");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_character_without_a_glyph_takes_a_narrow_space() {
        let path = sample("width");
        let mut font = Unifont::new(&path);
        assert_eq!(font.glyph('B'), None);
        assert_eq!(font.glyph('😀'), None, "beyond the plane");
        assert_eq!(font.width("A中B"), 8 + 16 + MISSING);
        assert!(font.take_problem().is_none());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_missing_or_foreign_file_is_unusable_and_says_why_once() {
        let mut font = Unifont::new("/nonexistent/unifont.hex");
        assert!(!font.usable());
        assert_eq!(font.glyph('A'), None);
        assert!(font
            .take_problem()
            .unwrap()
            .contains("/nonexistent/unifont.hex"));
        assert!(font.take_problem().is_none());

        let path = std::env::temp_dir().join(format!("unifont-notfont-{}", std::process::id()));
        std::fs::write(&path, "hello\nworld\n").unwrap();
        let mut font = Unifont::new(&path);
        assert!(!font.usable());
        assert!(font.take_problem().unwrap().contains("not a Unifont"));
        std::fs::remove_file(&path).unwrap();
    }
}
