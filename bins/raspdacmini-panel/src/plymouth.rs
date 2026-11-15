//! Whether the system's boot splash has the screen.
//!
//! Plymouth draws on framebuffers from the first moments of a boot until
//! the system is up, again while the system goes down, and through a
//! system update. While it runs, a panel that is a framebuffer is its to
//! draw on, and this program leaves the glass alone.
//!
//! Its daemon listens on a socket with a fixed name for as long as it
//! runs, so whether that socket can be reached is whether it is running.

use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixStream};

/// The name of the daemon's socket, in the abstract namespace.
const SOCKET: &[u8] = b"/org/freedesktop/plymouthd";

/// True while Plymouth's daemon is running.
pub fn running() -> bool {
    listening(SOCKET)
}

/// True where the system will bring its splash up as it shuts down: it
/// does where the kernel was started with `splash`, and without `nosplash`
/// or `plymouth.enable=0`, which is what the system's own units for it ask.
pub fn expected() -> bool {
    std::fs::read_to_string("/proc/cmdline").is_ok_and(|line| expected_in(&line))
}

fn expected_in(command_line: &str) -> bool {
    let has = |word: &str| command_line.split_whitespace().any(|w| w == word);
    has("splash") && !has("nosplash") && !has("plymouth.enable=0")
}

fn listening(name: &[u8]) -> bool {
    SocketAddr::from_abstract_name(name)
        .and_then(|address| UnixStream::connect_addr(&address))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn the_systems_splash_is_expected_where_the_kernel_was_started_with_it() {
        assert!(expected_in(
            "console=serial0,115200 root=/dev/mmcblk0p2 quiet splash nodebug\n"
        ));
        assert!(expected_in("splash"));
        assert!(!expected_in("console=tty1 root=/dev/mmcblk0p2 quiet"));
        assert!(!expected_in("quiet splash nosplash"));
        assert!(!expected_in("quiet splash plymouth.enable=0"));
        // A word that only contains it is not it.
        assert!(!expected_in(
            "plymouth.ignore-serial-consoles nosplashy splashscreen=1"
        ));
        assert!(!expected_in(""));
    }

    #[test]
    fn a_daemon_is_running_while_its_socket_answers() {
        let name = format!("/raspdacmini-test/plymouthd-{}", std::process::id());
        assert!(!listening(name.as_bytes()), "nobody listens yet");
        let address = SocketAddr::from_abstract_name(name.as_bytes()).unwrap();
        let listener = UnixListener::bind_addr(&address).unwrap();
        assert!(listening(name.as_bytes()));
        drop(listener);
        assert!(!listening(name.as_bytes()), "gone with its daemon");
    }
}
