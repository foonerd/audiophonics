//! The Audiophonics infrared handset, as the kernel presents it.
//!
//! The receiver hangs on a GPIO the `gpio-ir` overlay names, and the kernel
//! decodes what it hears: each frame the handset sends arrives on the
//! receiver's input device as a scancode, the first press and every repeat
//! while a button is held. This crate finds that device, sees that the NEC
//! decoder the handset needs is switched on, and turns scancodes into the
//! handset's buttons. No daemon sits in between and no key table is loaded
//! into the kernel.

pub mod keys;
pub mod rc;
pub mod reader;

pub use keys::Key;
pub use reader::{Event, Press};
