//! What every Audiophonics daemon needs whatever its display: the journal,
//! the plugin's settings store, the unit's address, local time and the idle
//! policy. Nothing here knows a panel or a player.

pub mod board;
pub mod clock;
pub mod idle;
pub mod log;
pub mod net;
pub mod settings;
