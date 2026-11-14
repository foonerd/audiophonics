//! The infrared receiver, found among the kernel's remote-control devices.
//!
//! Every such device has a directory under `/sys/class/rc`. The receiver on
//! a GPIO is the one driven by `gpio_ir_recv`; a Raspberry Pi also lists
//! its HDMI ports there, for the television's own remote. The directory
//! names the decoders the kernel can run for the device, the ones switched
//! on in square brackets, and holds the two devices its frames can be read
//! from: its own, which tells a key's frame from the repeats that follow
//! it, and an input device, which does not.

use std::path::{Path, PathBuf};

/// The driver of a receiver wired to a GPIO.
const DRIVER: &str = "gpio_ir_recv";
/// The decoder the handset needs, as the kernel names it: one name for
/// plain, extended and 32-bit NEC alike.
pub const PROTOCOL: &str = "nec";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receiver {
    /// The kernel's name for it, such as `rc0`.
    pub name: String,
    /// The file that lists its decoders and switches them.
    pub protocols: PathBuf,
    /// The receiver's own device, which hands over decoded frames.
    pub lirc: Option<PathBuf>,
    /// The input device its scancodes also arrive on.
    pub event: PathBuf,
}

pub fn find() -> Result<Receiver, String> {
    find_in(Path::new("/sys/class/rc"), Path::new("/dev"))
}

pub fn find_in(sys: &Path, dev: &Path) -> Result<Receiver, String> {
    let mut seen = Vec::new();
    for name in children(sys, "rc") {
        let dir = sys.join(&name);
        let driver = driver_of(&dir);
        if driver.as_deref() != Some(DRIVER) {
            seen.push(format!(
                "{name} ({})",
                driver.as_deref().unwrap_or("unknown")
            ));
            continue;
        }
        let event = children(&dir, "input")
            .into_iter()
            .flat_map(|input| {
                children(&dir.join(&input), "event")
                    .into_iter()
                    .map(|event| dev.join("input").join(event))
                    .collect::<Vec<_>>()
            })
            .next()
            .ok_or_else(|| format!("{name} has no input device"))?;
        return Ok(Receiver {
            protocols: dir.join("protocols"),
            lirc: children(&dir, "lirc").first().map(|lirc| dev.join(lirc)),
            name,
            event,
        });
    }
    Err(format!(
        "no infrared receiver ({DRIVER}) under {}; present: {}",
        sys.display(),
        if seen.is_empty() {
            "none".to_string()
        } else {
            seen.join(", ")
        }
    ))
}

/// The entries of a directory whose names start with `prefix`, in order.
fn children(dir: &Path, prefix: &str) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(prefix))
        .collect();
    names.sort();
    names
}

/// The driver a device says it has, in the `DRV_NAME` line of its `uevent`.
fn driver_of(dir: &Path) -> Option<String> {
    std::fs::read_to_string(dir.join("uevent"))
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("DRV_NAME="))
        .map(|name| name.trim().to_string())
}

/// The decoders switched on, from the text of a `protocols` file: the names
/// in square brackets.
pub fn enabled(text: &str) -> Vec<&str> {
    text.split_whitespace()
        .filter_map(|word| word.strip_prefix('[')?.strip_suffix(']'))
        .collect()
}

/// What became of asking for the handset's decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoder {
    /// It was on already.
    On,
    /// It was off, and has been switched on.
    SwitchedOn,
    /// It is off and could not be switched on; what the kernel has on, and
    /// why the request failed.
    Off { enabled: String, why: String },
}

/// See that the handset's decoder is on for this receiver, switching it on
/// where the file allows. The decoders already on are left as they are.
pub fn ensure_decoder(receiver: &Receiver) -> Decoder {
    let read = || std::fs::read_to_string(&receiver.protocols).unwrap_or_default();
    let before = read();
    if enabled(&before).contains(&PROTOCOL) {
        return Decoder::On;
    }
    let why = match std::fs::write(&receiver.protocols, format!("+{PROTOCOL}\n")) {
        Ok(()) => {
            if enabled(&read()).contains(&PROTOCOL) {
                return Decoder::SwitchedOn;
            }
            "the kernel did not take the request".to_string()
        }
        Err(e) => e.to_string(),
    };
    Decoder::Off {
        enabled: enabled(&before).join(" "),
        why,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(tag: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("audiophonics-rc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        // The television's remote on an HDMI port, then the receiver.
        let cec = root.join("rc0");
        std::fs::create_dir_all(cec.join("input3/event2")).unwrap();
        std::fs::write(
            cec.join("uevent"),
            "NAME=rc-cec\nDRV_NAME=cec\nDEV_NAME=vc4-hdmi-0\n",
        )
        .unwrap();
        let ir = root.join("rc1");
        std::fs::create_dir_all(ir.join("input5/event4")).unwrap();
        std::fs::create_dir_all(ir.join("lirc0")).unwrap();
        std::fs::write(
            ir.join("uevent"),
            "NAME=rc-rc6-mce\nDRV_NAME=gpio_ir_recv\nDEV_NAME=gpio_ir_recv\n",
        )
        .unwrap();
        std::fs::write(
            ir.join("protocols"),
            "rc-5 nec [rc-6] jvc sony rc-5-sz sanyo sharp mce_kbd xmp imon rc-mm [lirc]\n",
        )
        .unwrap();
        root
    }

    #[test]
    fn the_gpio_receiver_is_found_and_the_hdmi_port_is_passed_over() {
        let sys = tree("find");
        let r = find_in(&sys, Path::new("/dev")).unwrap();
        assert_eq!(r.name, "rc1");
        assert_eq!(r.event, PathBuf::from("/dev/input/event4"));
        assert_eq!(r.lirc, Some(PathBuf::from("/dev/lirc0")));
        assert_eq!(r.protocols, sys.join("rc1/protocols"));
        std::fs::remove_dir_all(sys.join("rc1/lirc0")).unwrap();
        assert_eq!(find_in(&sys, Path::new("/dev")).unwrap().lirc, None);
        std::fs::remove_dir_all(&sys).unwrap();
    }

    #[test]
    fn a_player_without_the_receiver_says_what_it_has() {
        let sys = tree("absent");
        std::fs::remove_dir_all(sys.join("rc1")).unwrap();
        let e = find_in(&sys, Path::new("/dev")).unwrap_err();
        assert!(e.contains("gpio_ir_recv") && e.contains("rc0 (cec)"), "{e}");
        let e = find_in(Path::new("/nonexistent/rc"), Path::new("/dev")).unwrap_err();
        assert!(e.contains("present: none"), "{e}");
        std::fs::remove_dir_all(&sys).unwrap();
    }

    #[test]
    fn the_decoders_switched_on_are_the_bracketed_ones() {
        assert_eq!(
            enabled("rc-5 nec [rc-6] jvc [lirc]\n"),
            vec!["rc-6", "lirc"]
        );
        assert_eq!(enabled("rc-5 [nec] rc-6 [lirc]"), vec!["nec", "lirc"]);
        assert!(enabled("").is_empty());
    }

    #[test]
    fn a_decoder_already_on_is_left_and_one_that_stays_off_is_explained() {
        let sys = tree("decoder");
        let r = find_in(&sys, Path::new("/dev")).unwrap();
        // A plain file takes the request as its whole content, which then
        // lists nothing in brackets: the request was written and not taken.
        match ensure_decoder(&r) {
            Decoder::Off { enabled, why } => {
                assert_eq!(enabled, "rc-6 lirc");
                assert!(why.contains("did not take"), "{why}");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(std::fs::read_to_string(&r.protocols).unwrap(), "+nec\n");
        std::fs::write(&r.protocols, "rc-5 [nec] rc-6 [lirc]\n").unwrap();
        assert_eq!(ensure_decoder(&r), Decoder::On);
        std::fs::remove_dir_all(&sys).unwrap();
    }
}
