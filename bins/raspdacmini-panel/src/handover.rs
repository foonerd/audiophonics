//! Devices that udev hands over.
//!
//! This program starts early in the boot, and can be running before udev
//! has given a device's node to the group the program runs in. The node is
//! then there and may not be opened. That passes within a second or two, so
//! an open that is refused for want of permission is tried again for a
//! short while before the refusal is taken as the answer. Any other refusal
//! is the answer at once.

use audiophonics_base::info;
use std::io;
use std::time::Duration;

/// How long a refusal for want of permission is waited out.
const WAIT: Duration = Duration::from_secs(3);
/// How long between two tries.
const STEP: Duration = Duration::from_millis(100);

/// Open the device called `what`, waiting out a refusal for want of
/// permission.
pub fn open<T>(what: &str, open: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let (answer, waited) = within(open, WAIT, STEP, std::thread::sleep);
    if answer.is_ok() && !waited.is_zero() {
        info!(
            "{what}: not open to this program for the first {} ms",
            waited.as_millis()
        );
    }
    answer
}

/// `open`, tried again `step` apart for as long as it is refused for want
/// of permission and `wait` has not gone. Returns the answer and how long
/// was waited for it.
pub fn within<T>(
    mut open: impl FnMut() -> io::Result<T>,
    wait: Duration,
    step: Duration,
    mut pause: impl FnMut(Duration),
) -> (io::Result<T>, Duration) {
    let mut waited = Duration::ZERO;
    loop {
        match open() {
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied && waited < wait => {
                pause(step);
                waited += step;
            }
            answer => return (answer, waited),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAIT: Duration = Duration::from_secs(3);
    const STEP: Duration = Duration::from_millis(100);

    /// An opener refused `refusals` times for `kind`, then opened; and how
    /// often it was tried and paused for.
    fn tried(refusals: u32, kind: io::ErrorKind) -> (io::Result<u32>, Duration, u32, u32) {
        let (mut tries, mut pauses) = (0, 0);
        let (answer, waited) = within(
            || {
                tries += 1;
                if tries <= refusals {
                    Err(io::Error::from(kind))
                } else {
                    Ok(tries)
                }
            },
            WAIT,
            STEP,
            |_| pauses += 1,
        );
        (answer, waited, tries, pauses)
    }

    #[test]
    fn a_device_that_opens_is_not_waited_for() {
        let (answer, waited, tries, pauses) = tried(0, io::ErrorKind::PermissionDenied);
        assert_eq!(answer.unwrap(), 1);
        assert_eq!((waited, tries, pauses), (Duration::ZERO, 1, 0));
    }

    #[test]
    fn a_device_not_handed_over_yet_is_waited_for() {
        let (answer, waited, tries, pauses) = tried(4, io::ErrorKind::PermissionDenied);
        assert_eq!(answer.unwrap(), 5);
        assert_eq!((waited, tries, pauses), (STEP * 4, 5, 4));
    }

    #[test]
    fn a_device_never_handed_over_is_refused_when_the_wait_has_gone() {
        let (answer, waited, tries, pauses) = tried(u32::MAX, io::ErrorKind::PermissionDenied);
        assert_eq!(answer.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(waited, WAIT);
        assert_eq!((tries, pauses), (31, 30));
    }

    #[test]
    fn any_other_refusal_is_the_answer_at_once() {
        for kind in [io::ErrorKind::NotFound, io::ErrorKind::Other] {
            let (answer, waited, tries, pauses) = tried(u32::MAX, kind);
            assert_eq!(answer.unwrap_err().kind(), kind);
            assert_eq!((waited, tries, pauses), (Duration::ZERO, 1, 0));
        }
    }

    #[test]
    fn a_refusal_of_another_kind_ends_the_wait() {
        let mut tries = 0;
        let (answer, waited) = within(
            || -> io::Result<()> {
                tries += 1;
                Err(io::Error::from(if tries < 3 {
                    io::ErrorKind::PermissionDenied
                } else {
                    io::ErrorKind::NotFound
                }))
            },
            WAIT,
            STEP,
            |_| {},
        );
        assert_eq!(answer.unwrap_err().kind(), io::ErrorKind::NotFound);
        assert_eq!((waited, tries), (STEP * 2, 3));
    }
}
