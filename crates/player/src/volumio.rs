//! Volumio implementation of [`Player`], over the REST interface the
//! backend serves on loopback.

use crate::http;
use crate::{Error, Player, Result, Seek, State, Transport};
use std::time::Duration;

#[derive(Clone)]
pub struct Volumio {
    base: String,
    timeout: Duration,
}

impl Volumio {
    pub fn new(base: impl Into<String>, timeout: Duration) -> Self {
        Self {
            base: base.into(),
            timeout,
        }
    }

    fn url(&self, suffix: &str) -> String {
        format!("{}{}", self.base, suffix)
    }

    fn command(&self, query: &str) -> Result<()> {
        http::get(
            &self.url(&format!("/api/v1/commands?cmd={query}")),
            self.timeout,
        )?;
        Ok(())
    }
}

impl Player for Volumio {
    fn state(&self) -> Result<State> {
        let body = http::get(&self.url("/api/v1/getState"), self.timeout)?;
        serde_json::from_str(&body).map_err(|e| Error::Decode(e.to_string()))
    }

    fn transport(&self, cmd: Transport) -> Result<()> {
        self.command(match cmd {
            Transport::Toggle => "toggle",
            Transport::Next => "next",
            Transport::Previous => "prev",
            Transport::Stop => "stop",
        })
    }

    fn seek(&self, step: Seek) -> Result<()> {
        self.command(match step {
            Seek::Forward => "seek&position=plus",
            Seek::Back => "seek&position=minus",
        })
    }

    fn set_volume(&self, percent: u8) -> Result<()> {
        self.command(&format!("volume&volume={}", percent.min(100)))
    }
}
