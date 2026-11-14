//! What every Audiophonics daemon needs whatever its display: the journal,
//! the plugin's settings store, the unit's address, local time, the idle
//! policy and what to do with the glass when the player leaves. Nothing
//! here knows a panel or a player.

pub mod board;
pub mod clock;
pub mod idle;
pub mod leaving;
pub mod log;
pub mod net;
pub mod settings;
