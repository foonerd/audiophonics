//! What a daemon says in the journal, by level.
//!
//! Error: a display or bus fault, a player that stopped answering, a
//! request the player refused, a device that could not be found. Info: what
//! changed: player state, remote keys, brightness. Debug: the raw input
//! events and the commands sent. The default is Error, so a unit in service
//! says nothing until something is wrong; the level is on the plugin page
//! for when a question needs answering.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error = 0,
    Info = 1,
    Debug = 2,
}

impl Level {
    pub fn parse(s: &str) -> Option<Level> {
        match s.trim().to_ascii_lowercase().as_str() {
            "error" => Some(Level::Error),
            "info" => Some(Level::Info),
            "debug" | "verbose" => Some(Level::Debug),
            _ => None,
        }
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Level::Error => "error",
            Level::Info => "info",
            Level::Debug => "debug",
        })
    }
}

static LEVEL: AtomicU8 = AtomicU8::new(Level::Error as u8);
static NAME: OnceLock<&'static str> = OnceLock::new();

pub fn set(level: Level) {
    LEVEL.store(level as u8, Ordering::Relaxed);
}

pub fn enabled(level: Level) -> bool {
    LEVEL.load(Ordering::Relaxed) >= level as u8
}

/// The name an error line starts with: the daemon's own. Set once, at start.
pub fn name_as(name: &'static str) {
    let _ = NAME.set(name);
}

pub fn name() -> &'static str {
    NAME.get().copied().unwrap_or("audiophonics")
}

/// Always said: something is wrong.
#[macro_export]
macro_rules! error {
    ($($t:tt)*) => { eprintln!("{}: {}", $crate::log::name(), format_args!($($t)*)) };
}

/// Said at Info and above: something changed.
#[macro_export]
macro_rules! info {
    ($($t:tt)*) => { if $crate::log::enabled($crate::log::Level::Info) { println!($($t)*) } };
}

/// Said at Debug only.
#[macro_export]
macro_rules! debug {
    ($($t:tt)*) => { if $crate::log::enabled($crate::log::Level::Debug) { println!($($t)*) } };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_parse_and_order() {
        assert_eq!(Level::parse("Info"), Some(Level::Info));
        assert_eq!(Level::parse("verbose"), Some(Level::Debug));
        assert_eq!(Level::parse("loud"), None);
        assert!(Level::Debug > Level::Info && Level::Info > Level::Error);
        set(Level::Info);
        assert!(enabled(Level::Info) && enabled(Level::Error) && !enabled(Level::Debug));
        set(Level::Error);
        assert!(!enabled(Level::Info));
    }

    #[test]
    fn errors_carry_a_name_until_the_daemon_gives_its_own() {
        assert!(!name().is_empty());
    }
}
