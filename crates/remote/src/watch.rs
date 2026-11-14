//! The receiver as a daemon keeps it while it runs.
//!
//! At boot a daemon may be up before the receiver is; a receiver's decoder
//! may be off and come on a moment later; a receiver may stop answering.
//! None of that stops a display. The loop looks at the receiver when it is
//! due, says in the journal what became of it when that changes, and
//! carries on without the remote meanwhile.

use crate::{rc, reader, Event, Reading};
use audiophonics_base::{error, info};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// What is said of a remote the settings page has switched off.
pub const OFF: &str = "switched off on the settings page";

/// How often a receiver that is not there yet, or a decoder that is off, is
/// looked at again.
pub const LOOK_AGAIN: Duration = Duration::from_secs(5);

/// The receiver as a daemon's loop keeps it: whether it is being read and
/// how, what there is to say of it, and when to look at it again.
pub struct Watch {
    receiver: Option<rc::Receiver>,
    reading: Option<Reading>,
    decoder_on: bool,
    line: String,
    look_again: Instant,
}

impl Watch {
    pub fn new(now: Instant) -> Self {
        Self {
            receiver: None,
            reading: None,
            decoder_on: false,
            line: "not looked for yet".to_string(),
            look_again: now,
        }
    }

    /// True when there is something to look at again: a receiver not yet
    /// read, or one whose decoder is off.
    pub fn due(&self, now: Instant) -> bool {
        now >= self.look_again && !(self.reading.is_some() && self.decoder_on)
    }

    /// Find the receiver and start reading it, or, where it is read
    /// already, see whether its decoder has come on. What became of it is
    /// said once, when it changes.
    ///
    /// `wrap` makes of the receiver's events what the loop is woken with.
    pub fn look<M: Send + 'static>(&mut self, tx: &Sender<M>, wrap: fn(Event) -> M, now: Instant) {
        self.look_again = now + LOOK_AGAIN;
        if self.reading.is_none() {
            if let Err(e) = self.start(tx, wrap) {
                self.say(e);
                return;
            }
        }
        let line = self.describe();
        self.say(line);
    }

    fn start<M: Send + 'static>(
        &mut self,
        tx: &Sender<M>,
        wrap: fn(Event) -> M,
    ) -> Result<(), String> {
        let receiver = rc::find().map_err(|e| format!("not found: {e}"))?;
        let reading = reader::spawn(&receiver, tx.clone(), wrap)
            .map_err(|e| format!("{} found, but it cannot be read: {e}", receiver.name))?;
        self.receiver = Some(receiver);
        self.reading = Some(reading);
        Ok(())
    }

    /// The receiver being read, in a sentence, with its decoder seen to.
    fn describe(&mut self) -> String {
        let (Some(receiver), Some(reading)) = (&self.receiver, &self.reading) else {
            return self.line.clone();
        };
        let mut line = match reading {
            Reading::Frames => format!(
                "reading {} at {}",
                receiver.name,
                receiver
                    .lirc
                    .as_deref()
                    .unwrap_or(&receiver.event)
                    .display()
            ),
            Reading::Scancodes { why } => format!(
                "reading {} at {} ({why}; a repeat with no frame before it reads as a press)",
                receiver.name,
                receiver.event.display()
            ),
        };
        match rc::ensure_decoder(receiver) {
            rc::Decoder::On | rc::Decoder::SwitchedOn => self.decoder_on = true,
            rc::Decoder::Off { enabled, why } => {
                self.decoder_on = false;
                line.push_str(&format!(
                    ", but its nec decoder is off (on: {enabled}) and could not be switched on: {why}"
                ));
            }
        }
        line
    }

    fn say(&mut self, line: String) {
        if line == self.line {
            return;
        }
        if self.reading.is_some() && self.decoder_on {
            info!("remote: {line}");
        } else {
            error!("remote: {line}");
        }
        self.line = line;
    }

    /// The device stopped answering: look for it afresh.
    pub fn lost(&mut self, why: &str, now: Instant) {
        self.receiver = None;
        self.reading = None;
        self.decoder_on = false;
        self.look_again = now + LOOK_AGAIN;
        self.say(format!("the receiver stopped answering: {why}"));
    }

    /// What there is to say of the receiver, in a sentence.
    pub fn line(&self) -> &str {
        &self.line
    }

    /// Look at the receiver again at once, not when it next falls due.
    pub fn look_now(&mut self, now: Instant) {
        self.look_again = now;
    }
}
