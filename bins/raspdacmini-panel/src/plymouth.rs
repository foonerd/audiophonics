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
