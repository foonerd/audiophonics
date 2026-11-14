//! What the panel does when the player is going down.
//!
//! The plugin tells the panel which way, from whichever surface asked, by
//! signal: USR1 for a halt, USR2 for a restart. On a restart the farewell
//! stays on the glass through the reboot, because the OLED keeps its picture
//! while the board resets, and the greeting replaces it when the runtime
//! returns. On a halt the panel says goodbye, holds it, and goes dark:
//! a halted board keeps the panel powered, and a lit farewell would sit
//! there until the mains are cut.

use std::time::Duration;

/// How long the goodbye stays before the panel goes dark on a halt.
pub const HOLD: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leaving {
    Off,
    Restart,
}

/// What to do with the glass when the process is told to stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtStop {
    /// A plain stop of the service: dark at once.
    Blank,
    /// A restart: leave the farewell up for the greeting to replace.
    Keep,
    /// A halt: hold the goodbye, then dark.
    HoldThenBlank,
}

pub fn at_stop(leaving: Option<Leaving>) -> AtStop {
    match leaving {
        None => AtStop::Blank,
        Some(Leaving::Restart) => AtStop::Keep,
        Some(Leaving::Off) => AtStop::HoldThenBlank,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_stop_blanks_a_restart_keeps_and_a_halt_holds_then_blanks() {
        assert_eq!(at_stop(None), AtStop::Blank);
        assert_eq!(at_stop(Some(Leaving::Restart)), AtStop::Keep);
        assert_eq!(at_stop(Some(Leaving::Off)), AtStop::HoldThenBlank);
    }
}
