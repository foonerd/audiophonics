//! The Audiophonics infrared handset, as the kernel presents it.
//!
//! The receiver hangs on a GPIO the `gpio-ir` overlay names, and the kernel
//! decodes what it hears: each frame the handset sends, the one for a key
//! going down and the short repeat frames that follow while it is held,
//! becomes a scancode. This crate finds the receiver, sees that the NEC
//! decoder the handset needs is switched on, reads the decoded frames, and
//! turns scancodes into the handset's buttons. No daemon sits in between
//! and no key table is loaded into the kernel.

pub mod keys;
pub mod rc;
pub mod reader;

pub use keys::Key;
pub use reader::{Event, Press, Reading};
