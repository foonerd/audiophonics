//! Fetching the cover of what plays, away from the loop that draws.
//!
//! The player names a cover by an address: a path on the player itself, or
//! an address elsewhere, often over HTTPS. Fetching it and turning it into
//! a background can take seconds and can fail in ways no display should
//! share: a file that is not a picture, a picture of a hundred megapixels,
//! a decoder that gives up. So it is done by a second process: this program
//! run again with `--cover`, which fetches with the player's own `curl`,
//! decodes, draws the background and writes its pixels out. Whatever
//! becomes of that process, the display is still there; a cover that could
//! not be made is a black background and a line in the journal.
//!
//! A worker thread runs that process for the cover wanted last and hands
//! the loop what came of it. A cover asked for while another is being made
//! replaces the one waiting: only the newest matters.

use crate::art::{self, Picture};
use crate::canvas::{Rgb, HEIGHT, WIDTH};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The bytes a finished background is: three for each pixel of the glass.
const BYTES: usize = (WIDTH * HEIGHT * 3) as usize;
/// How long a cover may take to come, and how large it may be.
const FETCH_SECONDS: u32 = 15;
const FETCH_BYTES: u32 = 24 * 1024 * 1024;
/// How long the whole making of a background may take.
const PATIENCE: Duration = Duration::from_secs(25);
/// How long after a failure the same cover is tried again: the second
/// time, the third, and every time after that.
const RETRY: [Duration; 3] = [
    Duration::from_secs(3),
    Duration::from_secs(10),
    Duration::from_secs(30),
];

/// The address to fetch for what the player calls a cover: a path is on
/// the player at `base`, anything with a scheme is taken as it is.
pub fn address(base: &str, albumart: &str) -> Option<String> {
    let albumart = albumart.trim();
    if albumart.is_empty() {
        return None;
    }
    let whole = if albumart.starts_with("http://") || albumart.starts_with("https://") {
        albumart.to_string()
    } else if albumart.starts_with('/') {
        format!("{}{albumart}", base.trim_end_matches('/'))
    } else {
        format!("{}/{albumart}", base.trim_end_matches('/'))
    };
    Some(encoded(&whole))
}

/// An address with everything in it that an address cannot hold written
/// as a percent and two digits. What is already written so is left.
fn encoded(address: &str) -> String {
    let mut out = String::with_capacity(address.len());
    for byte in address.bytes() {
        let plain = byte.is_ascii_alphanumeric() || b"-._~:/?#[]@!$&'()*+,;=%".contains(&byte);
        if plain {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Fetch a cover's file with the player's `curl`: its TLS, its
/// certificates, its redirects.
fn fetch(address: &str) -> Result<Vec<u8>, String> {
    let out = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--location",
            "--max-redirs",
            "5",
            "--proto",
            "=http,https",
            "--proto-redir",
            "=http,https",
        ])
        .args(["--max-time", &FETCH_SECONDS.to_string()])
        .args(["--max-filesize", &FETCH_BYTES.to_string()])
        .args(["--output", "-", "--"])
        .arg(address)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{address}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    if out.stdout.len() as u64 > u64::from(FETCH_BYTES) {
        return Err(format!(
            "{address}: {} bytes is too large",
            out.stdout.len()
        ));
    }
    Ok(out.stdout)
}

/// The background a cover makes, from the player's name for it.
fn make(base: &str, albumart: &str) -> Result<Vec<Rgb>, String> {
    let address = address(base, albumart).ok_or("no cover is named")?;
    let file = fetch(&address)?;
    let picture = Picture::open(&file).map_err(|e| format!("{address}: {e}"))?;
    Ok(art::backdrop(&picture))
}

/// What this program does when run with `--cover`: make the background and
/// write its pixels to standard output, red, green and blue for each.
pub fn helper(base: &str, albumart: &str) -> Result<(), String> {
    let px = make(base, albumart)?;
    let bytes: Vec<u8> = px.iter().flatten().copied().collect();
    std::io::stdout()
        .write_all(&bytes)
        .map_err(|e| format!("writing the background: {e}"))
}

/// The pixels a helper wrote, as a background.
fn read(bytes: &[u8]) -> Result<Vec<Rgb>, String> {
    if bytes.len() != BYTES {
        return Err(format!(
            "{} bytes of background where {BYTES} are a screen",
            bytes.len()
        ));
    }
    Ok(bytes.as_chunks::<3>().0.to_vec())
}

/// Run the helper for one cover and wait for it, but not for ever.
fn run_helper(program: &std::path::Path, base: &str, albumart: &str) -> Result<Vec<Rgb>, String> {
    let mut child = Command::new(program)
        .args(["--cover", albumart, "--player", base])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{}: {e}", program.display()))?;
    // Its output is read on threads of its own, so a helper that writes
    // more than a pipe holds is never left waiting on this one.
    let mut out = child.stdout.take();
    let mut err = child.stderr.take();
    let pixels = std::thread::spawn(move || {
        let mut bytes = Vec::with_capacity(BYTES);
        if let Some(pipe) = out.as_mut() {
            let _ = pipe.take(BYTES as u64 + 1).read_to_end(&mut bytes);
        }
        bytes
    });
    let said = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(pipe) = err.as_mut() {
            let _ = pipe.take(4096).read_to_string(&mut text);
        }
        text
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if started.elapsed() >= PATIENCE => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(format!("gave up after {} s", PATIENCE.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => break Err(e.to_string()),
        }
    };
    let bytes = pixels.join().unwrap_or_default();
    let said = said.join().unwrap_or_default();
    let status = status?;
    if !status.success() {
        let said = said.trim();
        return Err(if said.is_empty() {
            format!("the cover's helper ended with {status}")
        } else {
            said.to_string()
        });
    }
    read(&bytes)
}

/// What became of a cover that was wanted.
pub struct Arrived {
    /// The player's name for the cover, as it was asked for.
    pub albumart: String,
    pub backdrop: Result<Vec<Rgb>, String>,
}

/// What the worker and the loop share: the cover wanted next.
#[derive(Default)]
struct Wanted {
    albumart: Option<String>,
    closed: bool,
}

/// The worker that makes backgrounds.
pub struct Covers {
    shared: Arc<(Mutex<Wanted>, Condvar)>,
}

impl Covers {
    /// Start the worker. `deliver` is called on the worker's thread with
    /// each cover that was made or could not be.
    pub fn start(base: String, deliver: impl Fn(Arrived) + Send + 'static) -> Result<Self, String> {
        let program =
            std::env::current_exe().map_err(|e| format!("this program's own path: {e}"))?;
        let shared: Arc<(Mutex<Wanted>, Condvar)> = Arc::default();
        let theirs = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("covers".into())
            .spawn(move || {
                let (lock, bell) = &*theirs;
                loop {
                    let albumart = {
                        let Ok(mut wanted) = lock.lock() else { return };
                        loop {
                            if wanted.closed {
                                return;
                            }
                            if let Some(albumart) = wanted.albumart.take() {
                                break albumart;
                            }
                            let Ok(woken) = bell.wait(wanted) else { return };
                            wanted = woken;
                        }
                    };
                    let backdrop = run_helper(&program, &base, &albumart);
                    deliver(Arrived { albumart, backdrop });
                }
            })
            .map_err(|e| format!("the covers' thread: {e}"))?;
        Ok(Self { shared })
    }

    /// Ask for a cover. One asked for before and not yet begun is dropped.
    pub fn want(&self, albumart: &str) {
        let (lock, bell) = &*self.shared;
        if let Ok(mut wanted) = lock.lock() {
            wanted.albumart = Some(albumart.to_string());
            bell.notify_one();
        }
    }
}

impl Drop for Covers {
    fn drop(&mut self) {
        let (lock, bell) = &*self.shared;
        if let Ok(mut wanted) = lock.lock() {
            wanted.closed = true;
            bell.notify_one();
        }
    }
}

/// Which cover the screen has, which it is waiting for, and when one that
/// failed is due again.
pub struct Shelf {
    /// The player's name for the cover the screen should have.
    current: String,
    /// The background on the screen, and the name it was made from.
    shown: Option<(String, Vec<Rgb>)>,
    /// A cover asked for and not yet answered.
    asked: Option<String>,
    failures: usize,
    retry_at: Option<Instant>,
}

impl Default for Shelf {
    fn default() -> Self {
        Self::new()
    }
}

impl Shelf {
    pub fn new() -> Self {
        Self {
            current: String::new(),
            shown: None,
            asked: None,
            failures: 0,
            retry_at: None,
        }
    }

    /// The player's name for the cover of what plays now. Returns the name
    /// to ask the worker for, when there is one to ask for.
    pub fn playing(&mut self, albumart: &str, now: Instant) -> Option<String> {
        let albumart = albumart.trim();
        if albumart != self.current {
            self.current = albumart.to_string();
            self.failures = 0;
            self.retry_at = None;
            if albumart.is_empty() {
                // Nothing is named: nothing to wait for, nothing to show.
                self.shown = None;
            }
        }
        if self.current.is_empty() {
            return None;
        }
        let have = self
            .shown
            .as_ref()
            .is_some_and(|(name, _)| *name == self.current);
        let waiting = self.asked.as_deref() == Some(self.current.as_str());
        let due = self.retry_at.is_none_or(|at| now >= at);
        if have || waiting || !due {
            return None;
        }
        self.asked = Some(self.current.clone());
        self.retry_at = None;
        Some(self.current.clone())
    }

    /// What the worker made. Returns true when the screen's background
    /// changed, and what there is to say when a cover could not be made.
    pub fn arrived(&mut self, arrived: Arrived, now: Instant) -> (bool, Option<String>) {
        if self.asked.as_deref() == Some(arrived.albumart.as_str()) {
            self.asked = None;
        }
        if arrived.albumart != self.current {
            // The track moved on while this was being made.
            return (false, None);
        }
        match arrived.backdrop {
            Ok(px) => {
                self.failures = 0;
                self.retry_at = None;
                self.shown = Some((arrived.albumart, px));
                (true, None)
            }
            Err(why) => {
                let wait = RETRY[self.failures.min(RETRY.len() - 1)];
                self.failures += 1;
                self.retry_at = Some(now + wait);
                // The cover of the track before is not this track's.
                let cleared = self.shown.take().is_some();
                let said = (self.failures == 1).then(|| format!("cover: {why}"));
                (cleared, said)
            }
        }
    }

    /// The background for the screen: the cover's, or none.
    pub fn backdrop(&self) -> Option<&[Rgb]> {
        self.shown.as_ref().map(|(_, px)| px.as_slice())
    }

    /// When a cover that failed is to be tried again.
    pub fn retry_at(&self) -> Option<Instant> {
        self.retry_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_on_the_player_and_an_address_elsewhere_is_taken_as_it_is() {
        let base = "http://127.0.0.1:3000";
        assert_eq!(
            address(
                base,
                "/albumart?cacheid=1&web=Dire%20Straits/Brothers/large"
            ),
            Some(
                "http://127.0.0.1:3000/albumart?cacheid=1&web=Dire%20Straits/Brothers/large".into()
            )
        );
        assert_eq!(
            address("http://127.0.0.1:3000/", "albumart"),
            Some("http://127.0.0.1:3000/albumart".into())
        );
        assert_eq!(
            address(base, "https://i.scdn.co/image/ab67616d0000b273"),
            Some("https://i.scdn.co/image/ab67616d0000b273".into())
        );
        assert_eq!(address(base, ""), None);
        assert_eq!(address(base, "  "), None);
    }

    #[test]
    fn what_an_address_cannot_hold_is_written_as_percents_once() {
        assert_eq!(
            encoded("http://x/albumart?web=Dire Straits/Brothers In Arms/large&path=/mnt/a \"b\""),
            "http://x/albumart?web=Dire%20Straits/Brothers%20In%20Arms/large&path=/mnt/a%20%22b%22"
        );
        assert_eq!(encoded("http://x/a%20b"), "http://x/a%20b", "not twice");
        assert_eq!(encoded("http://x/\u{6771}"), "http://x/%E6%9D%B1");
    }

    #[test]
    fn a_background_is_as_many_bytes_as_a_screen_and_nothing_else_is_one() {
        let px = read(&vec![7u8; BYTES]).unwrap();
        assert_eq!(px.len(), 320 * 240);
        assert_eq!(px[0], [7, 7, 7]);
        assert!(read(&[]).unwrap_err().contains("0 bytes"));
        assert!(read(&vec![0u8; BYTES + 1]).is_err());
    }

    fn made(albumart: &str) -> Arrived {
        Arrived {
            albumart: albumart.into(),
            backdrop: Ok(vec![[1, 2, 3]; 4]),
        }
    }

    fn failed(albumart: &str) -> Arrived {
        Arrived {
            albumart: albumart.into(),
            backdrop: Err("404".into()),
        }
    }

    #[test]
    fn a_cover_is_asked_for_once_and_shown_when_it_comes() {
        let t0 = Instant::now();
        let mut shelf = Shelf::new();
        assert_eq!(shelf.playing("", t0), None);
        assert_eq!(shelf.playing("/a", t0), Some("/a".into()));
        assert_eq!(shelf.playing("/a", t0), None, "being made");
        assert!(shelf.backdrop().is_none());
        assert_eq!(shelf.arrived(made("/a"), t0), (true, None));
        assert_eq!(shelf.backdrop().unwrap()[0], [1, 2, 3]);
        assert_eq!(shelf.playing("/a", t0), None, "it is there");
    }

    #[test]
    fn the_cover_before_stays_until_the_next_one_comes_or_fails() {
        let t0 = Instant::now();
        let mut shelf = Shelf::new();
        shelf.playing("/a", t0);
        shelf.arrived(made("/a"), t0);
        assert_eq!(shelf.playing("/b", t0), Some("/b".into()));
        assert!(shelf.backdrop().is_some(), "no black between two covers");
        // One made for a track since left behind changes nothing.
        assert_eq!(shelf.playing("/c", t0), Some("/c".into()));
        assert_eq!(shelf.arrived(made("/b"), t0), (false, None));
        // The one wanted fails: the old cover goes, and it is said once.
        let (changed, said) = shelf.arrived(failed("/c"), t0);
        assert!(changed);
        assert_eq!(said.as_deref(), Some("cover: 404"));
        assert!(shelf.backdrop().is_none());
        // A track with no cover named shows none.
        shelf.playing("/a", t0);
        shelf.arrived(made("/a"), t0);
        assert_eq!(shelf.playing("", t0), None);
        assert!(shelf.backdrop().is_none());
    }

    #[test]
    fn a_cover_that_failed_is_tried_again_later_and_later_still() {
        let t0 = Instant::now();
        let mut shelf = Shelf::new();
        shelf.playing("/a", t0);
        shelf.arrived(failed("/a"), t0);
        assert_eq!(shelf.retry_at(), Some(t0 + RETRY[0]));
        assert_eq!(shelf.playing("/a", t0 + Duration::from_secs(1)), None);
        let t1 = t0 + RETRY[0];
        assert_eq!(shelf.playing("/a", t1), Some("/a".into()));
        let (_, said) = shelf.arrived(failed("/a"), t1);
        assert_eq!(said, None, "said the first time only");
        assert_eq!(shelf.retry_at(), Some(t1 + RETRY[1]));
        let t2 = t1 + RETRY[1];
        shelf.playing("/a", t2);
        shelf.arrived(failed("/a"), t2);
        let t3 = t2 + RETRY[2];
        shelf.playing("/a", t3);
        shelf.arrived(failed("/a"), t3);
        assert_eq!(
            shelf.retry_at(),
            Some(t3 + RETRY[2]),
            "every thirty seconds from there"
        );
        // It comes at last.
        let t4 = t3 + RETRY[2];
        assert_eq!(shelf.playing("/a", t4), Some("/a".into()));
        assert_eq!(shelf.arrived(made("/a"), t4), (true, None));
        assert_eq!(shelf.retry_at(), None);
        // Another track starts with a clean slate.
        shelf.playing("/b", t4);
        shelf.arrived(failed("/b"), t4);
        assert_eq!(shelf.retry_at(), Some(t4 + RETRY[0]));
    }

    /// A player on loopback that serves one file, for as long as the test
    /// keeps it.
    fn serving(body: Vec<u8>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut request = [0u8; 2048];
                let n = stream.read(&mut request).unwrap_or(0);
                let found = String::from_utf8_lossy(&request[..n]).starts_with("GET /albumart");
                let head = if found {
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                } else {
                    "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .to_string()
                };
                let _ = stream.write_all(head.as_bytes());
                if found {
                    let _ = stream.write_all(&body);
                }
            }
        });
        base
    }

    #[test]
    fn a_cover_is_fetched_from_the_player_and_made_into_a_background() {
        if Command::new("curl").arg("--version").output().is_err() {
            eprintln!("curl is not installed here: the fetch is not tried");
            return;
        }
        let cover = art::sample(120, 120);
        let raw: Vec<u8> = cover.px.iter().flatten().copied().collect();
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbImage::from_raw(120, 120, raw)
            .unwrap()
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let base = serving(png.into_inner());
        let px = make(&base, "/albumart?web=a b/large").unwrap();
        assert_eq!(px, art::backdrop(&cover));
        // What is not there is an error with the address in it.
        let said = make(&base, "/elsewhere").unwrap_err();
        assert!(
            said.contains("/elsewhere") && said.contains("404"),
            "{said}"
        );
        // What is there and is no picture is an error too.
        let base = serving(b"<html>hello</html>".to_vec());
        assert!(make(&base, "/albumart").is_err());
        assert!(make(&base, "").unwrap_err().contains("no cover"));
    }
}
