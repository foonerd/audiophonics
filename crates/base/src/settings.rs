//! The settings, as a plugin keeps them in Volumio's configuration store: a
//! JSON object of `key: { type, value }` entries, in
//! `/data/configuration/<category>/<plugin>/config.json`. A daemon reads
//! that file itself, at start and when the plugin asks, so there is one
//! source of truth.
//!
//! Every key has a default in the daemon. A file that is missing or short
//! of a key leaves the default; a value of the wrong kind is named and
//! leaves the default too, because a display must come up on a damaged
//! store and say what was wrong with it.

use serde_json::{Map, Value};
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct Store {
    values: Map<String, Value>,
}

impl Store {
    /// The store at `path`. A missing file is an empty store; a file that
    /// cannot be read or is not a JSON object is an error.
    pub fn load(path: impl AsRef<Path>) -> Result<Store, String> {
        let path = path.as_ref();
        match std::fs::read_to_string(path) {
            Ok(text) => Store::parse(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Store::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn parse(text: &str) -> Result<Store, String> {
        match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(values)) => Ok(Store { values }),
            Ok(_) => Err("not a JSON object".to_string()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// The value under a key: the `value` member of the store's entry, or
    /// the entry itself where a file was written by hand without one.
    fn raw(&self, key: &str) -> Option<&Value> {
        let entry = self.values.get(key)?;
        match entry {
            Value::Object(o) => o.get("value"),
            other => Some(other),
        }
    }

    /// A text value. A number or a switch reads as its text; null and an
    /// empty string are absent.
    pub fn text(&self, key: &str) -> Option<String> {
        match self.raw(key)? {
            Value::Null => None,
            Value::String(s) if s.trim().is_empty() => None,
            Value::String(s) => Some(s.trim().to_string()),
            other => Some(other.to_string()),
        }
    }

    /// A whole number, written as a number or as its digits in a string.
    pub fn integer(&self, key: &str) -> Result<Option<i64>, String> {
        let bad = |v: &Value| format!("{key}: not a whole number: {v}");
        match self.raw(key) {
            None | Some(Value::Null) => Ok(None),
            Some(v @ Value::Number(n)) => match n.as_i64() {
                Some(i) => Ok(Some(i)),
                None => match n.as_f64() {
                    Some(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => Ok(Some(f as i64)),
                    _ => Err(bad(v)),
                },
            },
            Some(v @ Value::String(s)) => {
                let s = s.trim();
                if s.is_empty() {
                    return Ok(None);
                }
                s.parse::<i64>().map(Some).map_err(|_| bad(v))
            }
            Some(v) => Err(bad(v)),
        }
    }

    /// A whole number within `min..=max`.
    pub fn integer_in(&self, key: &str, min: i64, max: i64) -> Result<Option<i64>, String> {
        match self.integer(key)? {
            Some(v) if v < min || v > max => Err(format!("{key}: {v} is outside {min} to {max}")),
            other => Ok(other),
        }
    }

    /// A switch, written as a boolean or as "true" or "false".
    pub fn switch(&self, key: &str) -> Result<Option<bool>, String> {
        match self.raw(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Bool(b)) => Ok(Some(*b)),
            Some(Value::String(s)) if s.trim() == "true" => Ok(Some(true)),
            Some(Value::String(s)) if s.trim() == "false" => Ok(Some(false)),
            Some(v) => Err(format!("{key}: not true or false: {v}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"{
        "contrast": { "type": "number", "value": 100 },
        "sleep_after": { "type": "number", "value": "1200" },
        "remote_active": { "type": "boolean", "value": true },
        "oled_active": { "type": "boolean", "value": "false" },
        "log_level": { "type": "string", "value": " info " },
        "empty": { "type": "string", "value": "" },
        "bare": 7,
        "half": { "type": "number", "value": 1.5 },
        "whole": { "type": "number", "value": 12.0 },
        "word": { "type": "number", "value": "soon" }
    }"#;

    #[test]
    fn values_read_from_the_store_as_volumio_writes_it() {
        let s = Store::parse(FILE).unwrap();
        assert_eq!(s.integer("contrast"), Ok(Some(100)));
        assert_eq!(
            s.integer("sleep_after"),
            Ok(Some(1200)),
            "digits in a string"
        );
        assert_eq!(s.switch("remote_active"), Ok(Some(true)));
        assert_eq!(s.switch("oled_active"), Ok(Some(false)), "a switch as text");
        assert_eq!(s.text("log_level").as_deref(), Some("info"));
        assert_eq!(s.text("contrast").as_deref(), Some("100"));
        assert_eq!(
            s.integer("bare"),
            Ok(Some(7)),
            "an entry without a value member"
        );
        assert_eq!(s.integer("whole"), Ok(Some(12)));
    }

    #[test]
    fn a_missing_key_is_absent_and_a_wrong_kind_is_named() {
        let s = Store::parse(FILE).unwrap();
        assert_eq!(s.integer("nothing"), Ok(None));
        assert_eq!(s.switch("nothing"), Ok(None));
        assert_eq!(s.text("nothing"), None);
        assert_eq!(s.text("empty"), None);
        assert!(s.integer("half").unwrap_err().contains("half"));
        assert!(s.integer("word").unwrap_err().contains("soon"));
        assert!(s.switch("contrast").unwrap_err().contains("contrast"));
        assert!(s
            .integer_in("contrast", 1, 50)
            .unwrap_err()
            .contains("outside 1 to 50"));
        assert_eq!(s.integer_in("contrast", 1, 255), Ok(Some(100)));
    }

    #[test]
    fn a_missing_file_is_an_empty_store_and_a_damaged_one_is_an_error() {
        let s = Store::load("/nonexistent/audiophonics/config.json").unwrap();
        assert_eq!(s.integer("contrast"), Ok(None));
        assert!(Store::parse("[1, 2]").unwrap_err().contains("object"));
        assert!(Store::parse("{ nope").is_err());
    }
}
