//! The receiver's input device, read on its own thread.
//!
//! The kernel reports every frame it decodes as a scancode: one when a
//! button goes down and one for each repeat the handset sends while it is
//! held, about nine a second. A scancode that follows the same one closely
//! is a repeat; anything else is a fresh press.

use crate::keys::{self, Key};
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

const EV_MSC: u16 = 4;
const MSC_SCAN: u16 = 4;

/// The kernel's input_event: two machine words of time, then type, code and
/// value. Sixteen bytes for a 32-bit process, whatever the kernel.
const WORD: usize = size_of::<usize>();
const RECORD: usize = 2 * WORD + 8;

/// A held button repeats every 110 ms; a gap well beyond that is a release.
const RELEASE_GAP: Duration = Duration::from_millis(300);

/// One frame from the handset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Press {
    pub scancode: u32,
    /// The button, when the scancode is one of the handset's.
    pub key: Option<Key>,
    /// 0 for the press itself, then 1, 2, 3 for each repeat while held.
    pub repeat: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Press(Press),
    /// The device stopped answering; no more events will come from it.
    Lost(String),
}

/// Tells a press from the repeats that follow it.
#[derive(Debug, Default)]
pub struct Tracker {
    last: Option<(u32, Instant)>,
    repeat: u32,
}

impl Tracker {
    pub fn scan(&mut self, scancode: u32, now: Instant) -> Press {
        let held = matches!(self.last, Some((code, at))
            if code == scancode && now.saturating_duration_since(at) <= RELEASE_GAP);
        self.repeat = if held { self.repeat + 1 } else { 0 };
        self.last = Some((scancode, now));
        Press {
            scancode,
            key: keys::key(scancode),
            repeat: self.repeat,
        }
    }
}

/// The scancode in one input record, if it carries one.
pub fn scancode(record: &[u8; RECORD]) -> Option<u32> {
    let field = |at: usize| u16::from_le_bytes([record[at], record[at + 1]]);
    let value = &record[2 * WORD + 4..];
    (field(2 * WORD) == EV_MSC && field(2 * WORD + 2) == MSC_SCAN)
        .then(|| u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

/// Read the device until it fails, sending each frame, wrapped by `wrap`
/// into whatever the caller's channel carries. A read failure is sent as
/// `Lost` and ends the thread.
pub fn spawn<M: Send + 'static>(
    path: PathBuf,
    tx: Sender<M>,
    wrap: fn(Event) -> M,
) -> Result<(), String> {
    let mut file = File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    std::thread::Builder::new()
        .name("remote".to_string())
        .spawn(move || {
            let mut tracker = Tracker::default();
            let mut record = [0u8; RECORD];
            loop {
                if let Err(e) = file.read_exact(&mut record) {
                    let _ = tx.send(wrap(Event::Lost(e.to_string())));
                    return;
                }
                if let Some(code) = scancode(&record) {
                    let press = tracker.scan(code, Instant::now());
                    if tx.send(wrap(Event::Press(press))).is_err() {
                        return;
                    }
                }
            }
        })
        .map_err(|e| format!("remote thread: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(kind: u16, code: u16, value: u32) -> [u8; RECORD] {
        let mut b = [0u8; RECORD];
        b[2 * WORD..2 * WORD + 2].copy_from_slice(&kind.to_le_bytes());
        b[2 * WORD + 2..2 * WORD + 4].copy_from_slice(&code.to_le_bytes());
        b[2 * WORD + 4..].copy_from_slice(&value.to_le_bytes());
        b
    }

    #[test]
    fn a_scancode_is_read_from_its_record_and_other_records_carry_none() {
        assert_eq!(
            scancode(&record(EV_MSC, MSC_SCAN, 0x80_191D)),
            Some(0x80_191D)
        );
        assert_eq!(scancode(&record(0, 0, 0)), None, "sync");
        assert_eq!(scancode(&record(1, 164, 1)), None, "a key");
        assert_eq!(scancode(&record(EV_MSC, 3, 7)), None, "another miscellany");
    }

    #[test]
    fn a_held_button_counts_its_repeats_and_a_pause_starts_over() {
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        let mut t = Tracker::default();
        let play = 0x80_191D;
        let first = t.scan(play, ms(0));
        assert_eq!((first.key, first.repeat), (Some(Key::Play), 0));
        assert_eq!(t.scan(play, ms(110)).repeat, 1);
        assert_eq!(t.scan(play, ms(220)).repeat, 2);
        assert_eq!(
            t.scan(play, ms(600)).repeat,
            0,
            "released and pressed again"
        );
        assert_eq!(t.scan(0x80_1951, ms(650)).repeat, 0, "another button");
        assert_eq!(t.scan(0x80_1951, ms(760)).repeat, 1);
    }

    #[test]
    fn a_scancode_from_another_handset_is_passed_on_without_a_button() {
        let p = Tracker::default().scan(0x1234, Instant::now());
        assert_eq!(p.key, None);
        assert_eq!(p.scancode, 0x1234);
    }
}
