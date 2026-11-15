//! The panel as the kernel presents it: a framebuffer.
//!
//! With the kernel's own driver for the panel loaded from the boot
//! configuration, the panel is a framebuffer from the first moments of the
//! boot, and whatever draws on framebuffers then draws on it: the player's
//! boot screens, and the progress of a system update, long before this
//! program can run. This program then draws into the same framebuffer.
//!
//! The framebuffer is found by the driver's name, not by its number: an
//! HDMI output or the firmware's own framebuffer can come before it or
//! after it.
//!
//! The kernel sends the panel the whole picture whenever any of it is
//! written, so only the rows that changed are written, and no more often
//! than the picture changes.

use crate::canvas::{Area, Frame, HEIGHT, WIDTH};
use std::fs::{File, OpenOptions};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

/// The name the kernel's driver gives the panel's framebuffer.
pub const PANEL: &str = "fb_ili9341";

const GRAPHICS: &str = "/sys/class/graphics";
const DEVICES: &str = "/dev";

pub struct FbDev {
    file: File,
    /// Bytes from the start of one row to the start of the next.
    stride: u64,
    description: String,
}

/// The framebuffer that is the panel, among those the kernel lists under
/// `graphics`: its name, such as `fb1`.
pub fn find_in(graphics: &Path) -> Result<String, String> {
    let mut others = Vec::new();
    let entries =
        std::fs::read_dir(graphics).map_err(|e| format!("{}: {e}", graphics.display()))?;
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| {
            name.strip_prefix("fb")
                .is_some_and(|n| n.parse::<u32>().is_ok())
        })
        .collect();
    names.sort();
    for name in names {
        let driver = std::fs::read_to_string(graphics.join(&name).join("name")).unwrap_or_default();
        let driver = driver.trim();
        if driver == PANEL {
            return Ok(name);
        }
        others.push(format!("{name} ({driver})"));
    }
    Err(format!(
        "no framebuffer named {PANEL}; present: {}; the panel's driver is loaded from the boot \
         configuration, which takes effect when the player has restarted",
        if others.is_empty() {
            "none".to_string()
        } else {
            others.join(", ")
        }
    ))
}

impl FbDev {
    /// Find the panel's framebuffer and open it.
    pub fn open() -> Result<Self, String> {
        Self::open_in(Path::new(GRAPHICS), Path::new(DEVICES))
    }

    pub fn open_in(graphics: &Path, devices: &Path) -> Result<Self, String> {
        let name = find_in(graphics)?;
        let sys = graphics.join(&name);
        let read = |what: &str| -> Result<String, String> {
            std::fs::read_to_string(sys.join(what))
                .map(|s| s.trim().to_string())
                .map_err(|e| format!("{name}: {what}: {e}"))
        };
        let size = read("virtual_size")?;
        if size != format!("{WIDTH},{HEIGHT}") {
            return Err(format!(
                "{name} is {size}, not {WIDTH},{HEIGHT}: the panel's driver was loaded turned another way"
            ));
        }
        let bits = read("bits_per_pixel")?;
        if bits != "16" {
            return Err(format!("{name} has {bits} bits a pixel, not 16"));
        }
        let stride: u64 = read("stride")?
            .parse()
            .map_err(|_| format!("{name}: its stride is not a number"))?;
        if stride < u64::from(WIDTH) * 2 {
            return Err(format!(
                "{name}: a stride of {stride} bytes is less than a row"
            ));
        }
        let path: PathBuf = devices.join(&name);
        let file = OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Self {
            file,
            stride,
            description: format!("{} ({PANEL}), {size}, {bits} bits a pixel", path.display()),
        })
    }

    pub fn describe(&self) -> &str {
        &self.description
    }

    /// Write the rows an area touches. The kernel sends the panel the whole
    /// picture after any write, so there is nothing to gain from less than
    /// whole rows.
    pub fn show(&mut self, frame: &Frame, area: Area) -> Result<(), String> {
        if area.h == 0 || area.y + area.h > HEIGHT {
            return Err(format!("{area:?} is not an area of the glass"));
        }
        let row = WIDTH as usize * 2;
        let failed = |e: std::io::Error| format!("framebuffer write: {e}");
        if self.stride == row as u64 {
            return self
                .file
                .write_all_at(&frame.rows(area.y, area.h), u64::from(area.y) * self.stride)
                .map_err(failed);
        }
        // A framebuffer whose rows are longer than the picture's: a row at
        // a time, each at its own place.
        for y in area.y..area.y + area.h {
            self.file
                .write_all_at(&frame.rows(y, 1), u64::from(y) * self.stride)
                .map_err(failed)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{Canvas, WHITE};

    /// A kernel's list of framebuffers and its devices, in a folder: each
    /// framebuffer by number, driver name, size, bits and stride.
    fn system(tag: &str, list: &[(u32, &str, &str, &str, &str)]) -> (PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("raspdacmini-fb-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (graphics, devices) = (root.join("graphics"), root.join("dev"));
        std::fs::create_dir_all(&devices).unwrap();
        std::fs::create_dir_all(graphics.join("fbcon")).unwrap();
        for (number, name, size, bits, stride) in list {
            let sys = graphics.join(format!("fb{number}"));
            std::fs::create_dir_all(&sys).unwrap();
            std::fs::write(sys.join("name"), format!("{name}\n")).unwrap();
            std::fs::write(sys.join("virtual_size"), format!("{size}\n")).unwrap();
            std::fs::write(sys.join("bits_per_pixel"), format!("{bits}\n")).unwrap();
            std::fs::write(sys.join("stride"), format!("{stride}\n")).unwrap();
            std::fs::write(devices.join(format!("fb{number}")), b"").unwrap();
        }
        (graphics, devices)
    }

    fn done(graphics: &Path) {
        std::fs::remove_dir_all(graphics.parent().unwrap()).unwrap();
    }

    #[test]
    fn the_panel_is_found_by_its_name_whatever_its_number() {
        let (graphics, devices) = system(
            "find",
            &[
                (0, "BCM2708 FB", "640,480", "32", "2560"),
                (1, "fb_ili9341", "320,240", "16", "640"),
            ],
        );
        assert_eq!(find_in(&graphics), Ok("fb1".to_string()));
        let fb = FbDev::open_in(&graphics, &devices).unwrap();
        assert!(fb.describe().contains("fb1") && fb.describe().contains("320,240"));
        done(&graphics);

        let (graphics, _) = system("first", &[(0, "fb_ili9341", "320,240", "16", "640")]);
        assert_eq!(find_in(&graphics), Ok("fb0".to_string()));
        done(&graphics);
    }

    #[test]
    fn a_system_without_the_panel_says_what_it_has_and_what_to_do() {
        let (graphics, devices) = system("none", &[(0, "vc4drmfb", "640,480", "16", "1280")]);
        let said = FbDev::open_in(&graphics, &devices).err().unwrap();
        assert!(said.contains("no framebuffer named fb_ili9341"), "{said}");
        assert!(said.contains("fb0 (vc4drmfb)"), "{said}");
        assert!(said.contains("restarted"), "{said}");
        done(&graphics);
        assert!(find_in(Path::new("/nonexistent/graphics")).is_err());
    }

    #[test]
    fn a_panel_of_another_shape_is_refused() {
        for (size, bits, stride, why) in [
            ("240,320", "16", "480", "turned another way"),
            ("320,240", "32", "1280", "32 bits a pixel"),
            ("320,240", "16", "320", "less than a row"),
            ("320,240", "16", "wide", "not a number"),
        ] {
            let (graphics, devices) = system("shape", &[(1, "fb_ili9341", size, bits, stride)]);
            let said = FbDev::open_in(&graphics, &devices).err().unwrap();
            assert!(said.contains(why), "{said}");
            done(&graphics);
        }
    }

    #[test]
    fn the_rows_an_area_touches_are_written_where_they_belong() {
        let (graphics, devices) = system("write", &[(1, "fb_ili9341", "320,240", "16", "640")]);
        let mut fb = FbDev::open_in(&graphics, &devices).unwrap();
        let mut canvas = Canvas::new();
        canvas.set(3, 100, WHITE);
        canvas.set(5, 101, [255, 0, 0]);
        let frame = Frame::from(&canvas);
        fb.show(&frame, Area::new(3, 100, 3, 2)).unwrap();
        let written = std::fs::read(devices.join("fb1")).unwrap();
        assert_eq!(
            written.len(),
            102 * 640,
            "up to the end of the last row touched"
        );
        assert_eq!(written[100 * 640 + 6..100 * 640 + 8], [0xFF, 0xFF]);
        assert_eq!(
            written[101 * 640 + 10..101 * 640 + 12],
            [0x00, 0xF8],
            "low byte first"
        );
        assert!(written[..100 * 640].iter().all(|&b| b == 0));
        assert!(fb.show(&frame, Area::new(0, 239, 1, 2)).is_err());
        assert!(fb.show(&frame, Area::new(0, 0, 1, 0)).is_err());
        done(&graphics);
    }

    #[test]
    fn a_framebuffer_with_longer_rows_gets_each_row_at_its_own_place() {
        let (graphics, devices) = system("stride", &[(1, "fb_ili9341", "320,240", "16", "704")]);
        let mut fb = FbDev::open_in(&graphics, &devices).unwrap();
        let mut canvas = Canvas::new();
        canvas.set(0, 1, WHITE);
        canvas.set(0, 2, WHITE);
        fb.show(&Frame::from(&canvas), Area::new(0, 1, 1, 2))
            .unwrap();
        let written = std::fs::read(devices.join("fb1")).unwrap();
        assert_eq!(written[704..706], [0xFF, 0xFF]);
        assert_eq!(written[2 * 704..2 * 704 + 2], [0xFF, 0xFF]);
        assert_eq!(
            written[704 + 640..2 * 704],
            [0u8; 64],
            "the rest of the row is left"
        );
        done(&graphics);
    }
}
