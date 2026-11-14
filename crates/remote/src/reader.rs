//! The receiver, read on its own thread.
//!
//! The handset sends one frame when a key goes down and then, while the key
//! is held, a short repeat frame about nine times a second. A repeat frame
//! carries no key of its own: the kernel answers it with the scancode of the
//! last frame it decoded, whenever that was.
//!
//! The receiver's own device says which is which, so that is what is read:
//! a frame is a press, and a repeat counts only when it follows the frame
//! of its own key closely. A repeat with no frame before it, which is what
//! a key's first frame lost to noise leaves behind, or a neighbour's
//! handset, is passed over.
//!
//! Where that device cannot be had, the receiver's input device is read
//! instead. It reports frames and repeats alike as bare scancodes, so a
//! scancode that follows the same one closely is taken for a repeat and
//! anything else for a press; a repeat with no frame before it is then
//! indistinguishable from a press of the key last decoded.

use crate::keys::{self, Key};
use crate::rc::Receiver;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

const EV_MSC: u16 = 4;
const MSC_SCAN: u16 = 4;

/// The kernel's input_event: two machine words of time, then type, code and
/// value. Sixteen bytes for a 32-bit process, whatever the kernel.
const WORD: usize = size_of::<usize>();
const INPUT_RECORD: usize = 2 * WORD + 8;

/// The kernel's lirc_scancode: a 64-bit time, flags, protocol, keycode and
/// a 64-bit scancode.
const FRAME_RECORD: usize = 24;
/// Set in a lirc_scancode's flags for a repeat frame.
const FLAG_REPEAT: u16 = 2;
/// LIRC_SET_REC_MODE, and the mode in which the device hands over decoded
/// frames rather than raw pulse lengths.
const LIRC_SET_REC_MODE: u32 = 0x4004_6912;
const LIRC_MODE_SCANCODE: u32 = 8;

/// A held key repeats every 110 ms; a gap well beyond that is a release.
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

/// How the receiver is being read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reading {
    /// From its own device: frames and repeats told apart.
    Frames,
    /// From its input device, and why its own could not be used.
    Scancodes { why: String },
}

/// Tells a press from the repeats that follow it.
#[derive(Debug, Default)]
pub struct Tracker {
    last: Option<(u32, Instant)>,
    repeat: u32,
}

impl Tracker {
    fn press(&mut self, scancode: u32, now: Instant, repeat: u32) -> Press {
        self.repeat = repeat;
        self.last = Some((scancode, now));
        Press {
            scancode,
            key: keys::key(scancode),
            repeat,
        }
    }

    fn held(&self, scancode: u32, now: Instant) -> bool {
        matches!(self.last, Some((code, at))
            if code == scancode && now.saturating_duration_since(at) <= RELEASE_GAP)
    }

    /// A frame the kernel has named: a key's own frame is a press, and a
    /// repeat counts when the frame of its key came just before it.
    pub fn frame(&mut self, scancode: u32, repeat: bool, now: Instant) -> Option<Press> {
        if !repeat {
            return Some(self.press(scancode, now, 0));
        }
        self.held(scancode, now)
            .then(|| self.press(scancode, now, self.repeat + 1))
    }

    /// A bare scancode: a repeat when it follows the same one closely, a
    /// press otherwise.
    pub fn scan(&mut self, scancode: u32, now: Instant) -> Press {
        let repeat = if self.held(scancode, now) {
            self.repeat + 1
        } else {
            0
        };
        self.press(scancode, now, repeat)
    }
}

/// The scancode in one input record, if it carries one.
pub fn scancode(record: &[u8; INPUT_RECORD]) -> Option<u32> {
    let field = |at: usize| u16::from_le_bytes([record[at], record[at + 1]]);
    let value = &record[2 * WORD + 4..];
    (field(2 * WORD) == EV_MSC && field(2 * WORD + 2) == MSC_SCAN)
        .then(|| u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

/// The scancode in one record of the receiver's own device, and whether it
/// is a repeat frame. A scancode too long for the handset's protocol is not
/// one of its frames.
pub fn frame(record: &[u8; FRAME_RECORD]) -> Option<(u32, bool)> {
    let flags = u16::from_le_bytes([record[8], record[9]]);
    let mut wide = [0u8; 8];
    wide.copy_from_slice(&record[16..24]);
    let code = u32::try_from(u64::from_le_bytes(wide)).ok()?;
    Some((code, flags & FLAG_REPEAT != 0))
}

/// Ask the receiver's device for decoded frames.
#[allow(unsafe_code)]
fn decoded_frames(file: &File) -> std::io::Result<()> {
    let mode = LIRC_MODE_SCANCODE;
    // SAFETY: LIRC_SET_REC_MODE reads one u32 through the pointer, which
    // `mode` is and outlives the call; the descriptor is this file's.
    let rc = unsafe { libc::ioctl(file.as_raw_fd(), LIRC_SET_REC_MODE as _, &raw const mode) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn open_frames(path: &Path) -> Result<File, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    decoded_frames(&file).map_err(|e| format!("{}: decoded frames: {e}", path.display()))?;
    Ok(file)
}

/// Read fixed-size records until the device fails, sending what `decode`
/// makes of each, wrapped by `wrap` into whatever the caller's channel
/// carries. A read failure is sent as `Lost` and ends the thread.
fn read_on<M, const N: usize>(
    mut file: File,
    tx: Sender<M>,
    wrap: fn(Event) -> M,
    mut decode: impl FnMut(&[u8; N], Instant) -> Option<Press> + Send + 'static,
) -> Result<(), String>
where
    M: Send + 'static,
{
    std::thread::Builder::new()
        .name("remote".to_string())
        .spawn(move || {
            let mut record = [0u8; N];
            loop {
                if let Err(e) = file.read_exact(&mut record) {
                    let _ = tx.send(wrap(Event::Lost(e.to_string())));
                    return;
                }
                if let Some(press) = decode(&record, Instant::now()) {
                    if tx.send(wrap(Event::Press(press))).is_err() {
                        return;
                    }
                }
            }
        })
        .map(|_| ())
        .map_err(|e| format!("remote thread: {e}"))
}

/// Start reading the receiver: from its own device where that can be had,
/// from its input device otherwise. Says which.
pub fn spawn<M: Send + 'static>(
    receiver: &Receiver,
    tx: Sender<M>,
    wrap: fn(Event) -> M,
) -> Result<Reading, String> {
    let why = match &receiver.lirc {
        Some(path) => match open_frames(path) {
            Ok(file) => {
                let mut tracker = Tracker::default();
                read_on(file, tx, wrap, move |record: &[u8; FRAME_RECORD], now| {
                    let (code, repeat) = frame(record)?;
                    tracker.frame(code, repeat, now)
                })?;
                return Ok(Reading::Frames);
            }
            Err(e) => e,
        },
        None => "the receiver has no device of its own".to_string(),
    };
    let path = &receiver.event;
    let file = File::open(path).map_err(|e| format!("{}: {e}; and {why}", path.display()))?;
    let mut tracker = Tracker::default();
    read_on(file, tx, wrap, move |record: &[u8; INPUT_RECORD], now| {
        Some(tracker.scan(scancode(record)?, now))
    })?;
    Ok(Reading::Scancodes { why })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::mpsc;

    const PLAY: u32 = 0x80_191D;
    const RIGHT: u32 = 0x80_1951;

    fn input_record(kind: u16, code: u16, value: u32) -> [u8; INPUT_RECORD] {
        let mut b = [0u8; INPUT_RECORD];
        b[2 * WORD..2 * WORD + 2].copy_from_slice(&kind.to_le_bytes());
        b[2 * WORD + 2..2 * WORD + 4].copy_from_slice(&code.to_le_bytes());
        b[2 * WORD + 4..].copy_from_slice(&value.to_le_bytes());
        b
    }

    fn frame_record(scancode: u64, flags: u16) -> [u8; FRAME_RECORD] {
        let mut b = [0u8; FRAME_RECORD];
        b[0..8].copy_from_slice(&123_456_789u64.to_le_bytes());
        b[8..10].copy_from_slice(&flags.to_le_bytes());
        b[10..12].copy_from_slice(&9u16.to_le_bytes());
        b[16..24].copy_from_slice(&scancode.to_le_bytes());
        b
    }

    fn ms(t0: Instant, n: u64) -> Instant {
        t0 + Duration::from_millis(n)
    }

    #[test]
    fn a_scancode_is_read_from_an_input_record_and_other_records_carry_none() {
        assert_eq!(scancode(&input_record(EV_MSC, MSC_SCAN, PLAY)), Some(PLAY));
        assert_eq!(scancode(&input_record(0, 0, 0)), None, "sync");
        assert_eq!(scancode(&input_record(1, 164, 1)), None, "a key");
        assert_eq!(scancode(&input_record(EV_MSC, 3, 7)), None, "another kind");
    }

    #[test]
    fn a_frame_record_gives_its_scancode_and_whether_it_is_a_repeat() {
        assert_eq!(
            frame(&frame_record(u64::from(PLAY), 0)),
            Some((PLAY, false))
        );
        assert_eq!(frame(&frame_record(u64::from(PLAY), 2)), Some((PLAY, true)));
        assert_eq!(
            frame(&frame_record(u64::from(PLAY), 1)),
            Some((PLAY, false)),
            "the toggle flag is not a repeat"
        );
        assert_eq!(
            frame(&frame_record(1 << 40, 0)),
            None,
            "too long for the handset"
        );
    }

    #[test]
    fn a_frame_is_a_press_and_its_repeats_count_while_the_key_is_held() {
        let t0 = Instant::now();
        let mut t = Tracker::default();
        let first = t.frame(PLAY, false, ms(t0, 0)).unwrap();
        assert_eq!((first.key, first.repeat), (Some(Key::Play), 0));
        assert_eq!(t.frame(PLAY, true, ms(t0, 110)).unwrap().repeat, 1);
        assert_eq!(t.frame(PLAY, true, ms(t0, 220)).unwrap().repeat, 2);
        // Two quick presses are two frames, however close.
        assert_eq!(t.frame(RIGHT, false, ms(t0, 400)).unwrap().repeat, 0);
        assert_eq!(t.frame(RIGHT, false, ms(t0, 550)).unwrap().repeat, 0);
    }

    #[test]
    fn a_repeat_with_no_frame_before_it_is_passed_over() {
        let t0 = Instant::now();
        let mut t = Tracker::default();
        assert_eq!(t.frame(PLAY, true, ms(t0, 0)), None, "nothing came before");
        t.frame(PLAY, false, ms(t0, 100)).unwrap();
        // An hour on, a key's own frame is lost to noise and only its
        // repeats arrive: the kernel answers them with the last scancode.
        let later = ms(t0, 3_600_000);
        assert_eq!(t.frame(PLAY, true, later), None);
        assert_eq!(
            t.frame(PLAY, true, later + Duration::from_millis(110)),
            None
        );
        // A repeat of another key than the one held is not counted either.
        t.frame(RIGHT, false, later + Duration::from_millis(500))
            .unwrap();
        assert_eq!(
            t.frame(PLAY, true, later + Duration::from_millis(610)),
            None
        );
    }

    #[test]
    fn bare_scancodes_are_told_apart_by_how_closely_they_follow() {
        let t0 = Instant::now();
        let mut t = Tracker::default();
        assert_eq!(t.scan(PLAY, ms(t0, 0)).repeat, 0);
        assert_eq!(t.scan(PLAY, ms(t0, 110)).repeat, 1);
        assert_eq!(t.scan(PLAY, ms(t0, 220)).repeat, 2);
        assert_eq!(
            t.scan(PLAY, ms(t0, 600)).repeat,
            0,
            "released, pressed again"
        );
        assert_eq!(t.scan(RIGHT, ms(t0, 650)).repeat, 0, "another button");
        assert_eq!(t.scan(RIGHT, ms(t0, 760)).repeat, 1);
        let foreign = Tracker::default().scan(0x1234, t0);
        assert_eq!((foreign.key, foreign.scancode), (None, 0x1234));
    }

    #[test]
    fn a_device_of_its_own_that_cannot_be_used_falls_back_to_the_input_device() {
        let dir = std::env::temp_dir().join(format!("audiophonics-reader-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // A plain file refuses the request for decoded frames.
        let lirc = dir.join("lirc0");
        std::fs::write(&lirc, b"").unwrap();
        let event = dir.join("event4");
        let mut records = Vec::new();
        records.extend_from_slice(&input_record(EV_MSC, MSC_SCAN, PLAY));
        records.extend_from_slice(&input_record(0, 0, 0));
        records.extend_from_slice(&input_record(EV_MSC, MSC_SCAN, PLAY));
        std::fs::write(&event, records).unwrap();
        let receiver = Receiver {
            name: "rc0".into(),
            protocols: PathBuf::from("/nonexistent"),
            lirc: Some(lirc.clone()),
            event,
        };
        let (tx, rx) = mpsc::channel::<Event>();
        let reading = spawn(&receiver, tx, |e| e).unwrap();
        match reading {
            Reading::Scancodes { why } => assert!(why.contains("lirc0"), "{why}"),
            other => panic!("{other:?}"),
        }
        let wait = Duration::from_secs(5);
        let first = rx.recv_timeout(wait).unwrap();
        assert!(matches!(first, Event::Press(p) if p.key == Some(Key::Play) && p.repeat == 0));
        let second = rx.recv_timeout(wait).unwrap();
        assert!(matches!(second, Event::Press(p) if p.repeat == 1));
        assert!(
            matches!(rx.recv_timeout(wait).unwrap(), Event::Lost(_)),
            "end of file"
        );

        let none = Receiver {
            name: "rc0".into(),
            protocols: PathBuf::from("/nonexistent"),
            lirc: None,
            event: dir.join("absent"),
        };
        let (tx, _rx) = mpsc::channel::<Event>();
        let e = spawn(&none, tx, |e| e).unwrap_err();
        assert!(
            e.contains("absent") && e.contains("no device of its own"),
            "{e}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
