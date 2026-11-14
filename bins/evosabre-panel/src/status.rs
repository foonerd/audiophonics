//! What the panel knows about its remote, left where the plugin's settings
//! page can read it: a small JSON file in the service's runtime directory,
//! rewritten when the receiver is found or lost and when a button arrives.
//!
//! The page's "Check the remote" reads it, so the answer comes from the
//! running service and not from a second look at the hardware.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Status {
    file: Option<PathBuf>,
    receiver: String,
    last_key: Option<(String, u64)>,
}

impl Status {
    /// In the directory systemd gives the service; nowhere when run by hand.
    pub fn new() -> Self {
        Self::in_dir(std::env::var_os("RUNTIME_DIRECTORY").map(PathBuf::from))
    }

    pub fn in_dir(dir: Option<PathBuf>) -> Self {
        let status = Self {
            file: dir.map(|d| d.join("remote.json")),
            receiver: "not started".to_string(),
            last_key: None,
        };
        status.write();
        status
    }

    /// What became of finding the receiver, in a sentence.
    pub fn receiver(&mut self, line: impl Into<String>) {
        self.receiver = line.into();
        self.write();
    }

    /// A button, or a scancode that is not one of the handset's, just now.
    pub fn key(&mut self, name: impl Into<String>) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.last_key = Some((name.into(), now));
        self.write();
    }

    fn json(&self) -> String {
        let mut value = serde_json::json!({ "receiver": self.receiver });
        if let Some((key, at)) = &self.last_key {
            value["last_key"] = serde_json::json!(key);
            value["last_key_at"] = serde_json::json!(at);
        }
        format!("{value}\n")
    }

    fn write(&self) {
        if let Some(file) = &self.file {
            // The page can do without it; the panel must not stop for it.
            let _ = std::fs::write(file, self.json());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_says_what_the_receiver_is_doing_and_which_button_came_last() {
        let dir = std::env::temp_dir().join(format!("evosabre-status-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("remote.json");
        let read = || -> serde_json::Value {
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap()
        };
        let mut s = Status::in_dir(Some(dir.clone()));
        assert_eq!(read(), serde_json::json!({ "receiver": "not started" }));
        s.receiver("reading \"rc0\" at /dev/input/event4");
        s.key("play");
        let now = read();
        assert_eq!(now["receiver"], "reading \"rc0\" at /dev/input/event4");
        assert_eq!(now["last_key"], "play");
        assert!(now["last_key_at"].as_u64().unwrap() > 1_700_000_000);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn without_a_runtime_directory_nothing_is_written() {
        let mut s = Status::in_dir(None);
        s.receiver("anything");
        s.key("play");
        assert!(s.json().contains("\"last_key\":\"play\""));
    }
}
