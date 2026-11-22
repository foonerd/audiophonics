//! The player boundary.
//!
//! Everything above this trait, a display's layouts and a remote's key
//! map, is about a player in the abstract. Everything below it is about
//! Volumio specifically.

pub mod gate;
mod http;
pub mod probe;
pub mod volumio;

use serde::Deserialize;

pub use volumio::Volumio;

/// What is playing, as much as a small display can use.
///
/// Volumio's state is not the same shape for every source: a web radio
/// station reports `artist` and `album` as null and its sample rate as a
/// bare number where a file gives "96 kHz"; some services give the position
/// and the length as fractions or as text. Every field here takes what any
/// source sends, so no source can make the player look gone.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct State {
    #[serde(default, deserialize_with = "lenient_string")]
    pub status: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub title: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub artist: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub album: String,
    #[serde(default, deserialize_with = "lenient_percent")]
    pub volume: Option<u8>,
    #[serde(default, deserialize_with = "lenient_bool")]
    pub mute: bool,
    /// Sample rate as the player reports it, such as "192 kHz".
    #[serde(default, deserialize_with = "lenient_rate")]
    pub samplerate: String,
    #[serde(default, deserialize_with = "lenient_string")]
    pub bitdepth: String,
    /// Position in the track in milliseconds and its length in seconds, as
    /// Volumio reports them; absent or null for a stream.
    #[serde(default, deserialize_with = "lenient_count")]
    pub seek: Option<u64>,
    #[serde(default, deserialize_with = "lenient_count")]
    pub duration: Option<u64>,
    #[serde(default, deserialize_with = "lenient_bool")]
    pub repeat: bool,
    #[serde(default, rename = "repeatSingle", deserialize_with = "lenient_bool")]
    pub repeat_single: bool,
    #[serde(default, deserialize_with = "lenient_bool")]
    pub random: bool,
    /// Where the cover is: a path on the player, or an address elsewhere.
    #[serde(default, deserialize_with = "lenient_string")]
    pub albumart: String,
    /// The bit rate as the player words it, such as "320 Kbps"; empty for
    /// most files.
    #[serde(default, deserialize_with = "lenient_string")]
    pub bitrate: String,
    #[serde(default, deserialize_with = "lenient_count")]
    pub channels: Option<u64>,
    /// The kind of track: "flac", "mp3", "webradio", "spotify".
    #[serde(default, rename = "trackType", deserialize_with = "lenient_string")]
    pub track_type: String,
    /// The track's place in the queue, counted from nought.
    #[serde(default, deserialize_with = "lenient_count")]
    pub position: Option<u64>,
}

impl State {
    pub fn is_playing(&self) -> bool {
        self.status == "play"
    }

    /// True when the queue, or the track, plays again at its end.
    pub fn repeats(&self) -> bool {
        self.repeat || self.repeat_single
    }
}

/// null, a string, a number or a bool, as text; null is empty.
fn lenient_string<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    let v: Option<serde_json::Value> = Option::deserialize(d)?;
    Ok(match v {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::String(s)) => s,
        Some(other) => other.to_string(),
    })
}

/// A sample rate: "96 kHz" as a file gives it, or a bare number of kHz as
/// a stream gives it.
fn lenient_rate<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    let v: Option<serde_json::Value> = Option::deserialize(d)?;
    Ok(match v {
        None | Some(serde_json::Value::Null) => String::new(),
        Some(serde_json::Value::String(s)) => s,
        Some(serde_json::Value::Number(n)) => format!("{n} kHz"),
        Some(other) => other.to_string(),
    })
}

/// null is false; "true" is true.
fn lenient_bool<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<bool, D::Error> {
    let v: Option<serde_json::Value> = Option::deserialize(d)?;
    Ok(match v {
        Some(serde_json::Value::Bool(b)) => b,
        Some(serde_json::Value::String(s)) => s == "true",
        _ => false,
    })
}

/// A count that may arrive as a whole number, a fraction or text: whole
/// seconds or milliseconds, the fraction dropped. Anything else is absent.
fn loose_count(v: &serde_json::Value) -> Option<u64> {
    let f = match v {
        serde_json::Value::Number(n) => match n.as_u64() {
            Some(u) => return Some(u),
            None => n.as_f64()?,
        },
        serde_json::Value::String(s) => {
            let s = s.trim();
            match s.parse::<u64>() {
                Ok(u) => return Some(u),
                Err(_) => s.parse::<f64>().ok()?,
            }
        }
        _ => return None,
    };
    (f.is_finite() && f >= 0.0).then(|| f.trunc() as u64)
}

fn lenient_count<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<u64>, D::Error> {
    let v: Option<serde_json::Value> = Option::deserialize(d)?;
    Ok(v.as_ref().and_then(loose_count))
}

/// A volume: 0 to 100, whatever a source sends above it.
fn lenient_percent<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<u8>, D::Error> {
    let v: Option<serde_json::Value> = Option::deserialize(d)?;
    Ok(v.as_ref().and_then(loose_count).map(|n| n.min(100) as u8))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Toggle,
    Next,
    Previous,
    Stop,
}

/// A step through the track, of the length the player chooses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seek {
    Forward,
    Back,
}

/// What a handset asks of the player besides moving through the music:
/// each is a step or a switch whose outcome the player decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    VolumeUp,
    VolumeDown,
    /// Muted if it was not, and back.
    Mute,
    /// Repeat on if it was off, and back.
    Repeat,
    /// The queue in no order if it was in order, and back.
    Shuffle,
}

/// Where a player's own start is, in its own word. A player answers
/// requests well before it has finished starting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Readiness {
    /// Its web server is up; the rest of it is still starting.
    Starting,
    /// It has finished starting.
    Ready,
    /// It says its start failed.
    Failed,
    /// It has no word for it: it answered with something else.
    Unsaid,
}

impl Readiness {
    /// What Volumio answers at `/status`: `starting`, `ready` or `error`.
    /// A Volumio that does not serve that address answers with a page.
    pub fn parse(body: &str) -> Self {
        match body.trim() {
            "starting" => Self::Starting,
            "ready" => Self::Ready,
            "error" => Self::Failed,
            _ => Self::Unsaid,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Transport(std::io::Error),
    Decode(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Transport(e) => write!(f, "transport: {e}"),
            Error::Decode(m) => write!(f, "decode: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Transport(e)
    }
}

/// The whole surface a display and a remote need from a player.
pub trait Player {
    fn state(&self) -> Result<State>;
    fn transport(&self, cmd: Transport) -> Result<()>;
    fn seek(&self, step: Seek) -> Result<()>;
    fn set_volume(&self, percent: u8) -> Result<()>;
    fn ask(&self, what: Ask) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_players_word_for_its_start_is_one_of_three_or_none() {
        assert_eq!(Readiness::parse("starting"), Readiness::Starting);
        assert_eq!(Readiness::parse("ready\n"), Readiness::Ready);
        assert_eq!(Readiness::parse(" error "), Readiness::Failed);
        // A player that does not serve the address answers with its page.
        assert_eq!(Readiness::parse("<!doctype html><html>"), Readiness::Unsaid);
        assert_eq!(Readiness::parse(""), Readiness::Unsaid);
        assert_eq!(Readiness::parse("READY"), Readiness::Unsaid);
    }

    #[test]
    fn state_tolerates_a_sparse_body() {
        let s: State = serde_json::from_str(r#"{"status":"play","title":"x"}"#).unwrap();
        assert!(s.is_playing());
        assert_eq!(s.title, "x");
        assert_eq!(s.artist, "");
        assert_eq!(s.volume, None);
        assert!(!s.repeats() && !s.random);
    }

    #[test]
    fn a_web_radio_state_with_nulls_and_a_numeric_rate_decodes() {
        let body = r#"{"status":"play","position":0,"seek":1230,"duration":0,"samplerate":48,"bitdepth":"32 bit","channels":2,"random":false,"updatedb":false,"repeat":false,"bitrate":null,"isStreaming":false,"title":"bbc_6music-audio%3d128000.norewind.m3u8","artist":null,"album":null,"mute":null,"volume":45,"uri":"http://x/y.m3u8","trackType":"m3u8"}"#;
        let s: State = serde_json::from_str(body).unwrap();
        assert!(s.is_playing());
        assert_eq!(s.artist, "");
        assert_eq!(s.album, "");
        assert_eq!(s.samplerate, "48 kHz");
        assert_eq!(s.bitdepth, "32 bit");
        assert!(!s.mute);
        assert_eq!(s.volume, Some(45));
        assert_eq!(s.duration, Some(0));
    }

    #[test]
    fn the_cover_and_what_a_second_page_lists_decode() {
        let body = r#"{"status":"play","position":3,"albumart":"/albumart?cacheid=1&web=a/b/large","bitrate":"320 Kbps","channels":2,"trackType":"flac"}"#;
        let s: State = serde_json::from_str(body).unwrap();
        assert_eq!(s.albumart, "/albumart?cacheid=1&web=a/b/large");
        assert_eq!(s.bitrate, "320 Kbps");
        assert_eq!(s.channels, Some(2));
        assert_eq!(s.track_type, "flac");
        assert_eq!(s.position, Some(3));
        let bare: State = serde_json::from_str(
            r#"{"albumart":null,"bitrate":null,"channels":"","trackType":null}"#,
        )
        .unwrap();
        assert_eq!(bare.albumart, "");
        assert_eq!(bare.bitrate, "");
        assert_eq!(bare.channels, None);
        assert_eq!(bare.track_type, "");
        assert_eq!(bare.position, None);
    }

    #[test]
    fn fractions_and_text_for_position_length_and_volume_decode() {
        let body = r#"{"status":"play","seek":35080.993,"duration":"323.038","volume":"37","repeat":true,"repeatSingle":null,"random":"true"}"#;
        let s: State = serde_json::from_str(body).unwrap();
        assert_eq!(s.seek, Some(35080));
        assert_eq!(s.duration, Some(323));
        assert_eq!(s.volume, Some(37));
        assert!(s.repeats() && s.random);
        let odd: State =
            serde_json::from_str(r#"{"seek":-5,"duration":"soon","volume":250,"mute":"true"}"#)
                .unwrap();
        assert_eq!(odd.seek, None);
        assert_eq!(odd.duration, None);
        assert_eq!(odd.volume, Some(100));
        assert!(odd.mute);
    }
}
