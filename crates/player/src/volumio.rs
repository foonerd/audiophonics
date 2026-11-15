//! Volumio implementation of [`Player`], over the REST interface the
//! backend serves on loopback.

use crate::http;
use crate::{Ask, Error, Player, Readiness, Result, Seek, State, Transport};
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

    /// Ask the player for something. The player answers a command when
    /// what it started has settled, which a stream slow to open takes its
    /// time over; the request has gone out whole by then, so an answer that
    /// does not come in time is not a failure.
    fn command(&self, query: &str) -> Result<()> {
        let sent = http::send(
            &self.url(&format!("/api/v1/commands?cmd={query}")),
            self.timeout,
        )?;
        match http::reply(sent) {
            Ok(_) => Ok(()),
            Err(e) if http::timed_out(&e) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Ask a plugin for something through the endpoint it has registered
    /// with the player, with one word of what is asked. As with a command,
    /// an answer that does not come in time is not a failure: the request
    /// has gone out.
    pub fn plugin(&self, endpoint: &str, word: &str) -> Result<()> {
        let sent = http::send(
            &self.url(&format!(
                "/api/v1/pluginEndpoint?endpoint={endpoint}&data={word}"
            )),
            self.timeout,
        )?;
        match http::reply(sent) {
            Ok(_) => Ok(()),
            Err(e) if http::timed_out(&e) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Where the player's own start is, in its own word.
    pub fn readiness(&self) -> Result<Readiness> {
        let body = http::get(&self.url("/status"), self.timeout)?;
        Ok(Readiness::parse(&body))
    }

    /// How many tracks the queue holds.
    pub fn queue_length(&self) -> Result<u64> {
        let body = http::get(&self.url("/api/v1/getQueue"), self.timeout)?;
        let queue: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| Error::Decode(e.to_string()))?;
        queue
            .get("queue")
            .and_then(serde_json::Value::as_array)
            .map(|tracks| tracks.len() as u64)
            .ok_or_else(|| Error::Decode("the answer names no queue".to_string()))
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

    fn ask(&self, what: Ask) -> Result<()> {
        self.command(match what {
            Ask::VolumeUp => "volume&volume=plus",
            Ask::VolumeDown => "volume&volume=minus",
            Ask::Mute => "volume&volume=toggle",
            // Without a value the player turns each over.
            Ask::Repeat => "repeat",
            Ask::Shuffle => "random",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A player on loopback that takes one request and answers it with
    /// `body` after `delay`, or never when there is no body.
    fn player(
        body: Option<&'static str>,
        delay: Duration,
    ) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let served = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let n = stream.read(&mut request).unwrap();
            std::thread::sleep(delay);
            if let Some(body) = body {
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{body}"
                );
            }
            String::from_utf8_lossy(&request[..n]).into_owned()
        });
        (base, served)
    }

    #[test]
    fn the_state_is_read_from_the_players_answer() {
        let (base, served) = player(Some(r#"{"status":"play","title":"Aria"}"#), Duration::ZERO);
        let state = Volumio::new(base, Duration::from_millis(500))
            .state()
            .unwrap();
        assert_eq!(state.title, "Aria");
        assert!(served
            .join()
            .unwrap()
            .starts_with("GET /api/v1/getState HTTP/1.1"));
    }

    #[test]
    fn a_command_is_sent_as_the_player_names_it() {
        let (base, served) = player(Some(r#"{"response":"ok"}"#), Duration::ZERO);
        Volumio::new(base, Duration::from_millis(500))
            .seek(Seek::Forward)
            .unwrap();
        assert!(served
            .join()
            .unwrap()
            .starts_with("GET /api/v1/commands?cmd=seek&position=plus HTTP/1.1"));
    }

    #[test]
    fn a_step_or_a_switch_is_asked_for_and_the_player_decides_what_it_comes_to() {
        for (what, sent) in [
            (Ask::VolumeUp, "cmd=volume&volume=plus "),
            (Ask::VolumeDown, "cmd=volume&volume=minus "),
            (Ask::Mute, "cmd=volume&volume=toggle "),
            (Ask::Repeat, "cmd=repeat "),
            (Ask::Shuffle, "cmd=random "),
        ] {
            let (base, served) = player(Some(r#"{"response":"ok"}"#), Duration::ZERO);
            Volumio::new(base, Duration::from_millis(500))
                .ask(what)
                .unwrap();
            let request = served.join().unwrap();
            assert!(request.contains(sent), "{what:?}: {request}");
        }
    }

    #[test]
    fn a_plugin_is_asked_through_its_endpoint_and_the_queue_is_counted() {
        let (base, served) = player(Some(r#"{"success":true}"#), Duration::ZERO);
        Volumio::new(base, Duration::from_millis(500))
            .plugin("raspdac_mini_lcd", "shutdown")
            .unwrap();
        assert!(served.join().unwrap().starts_with(
            "GET /api/v1/pluginEndpoint?endpoint=raspdac_mini_lcd&data=shutdown HTTP/1.1"
        ));

        let (base, served) = player(
            Some(r#"{"queue":[{"name":"a"},{"name":"b"},{"name":"c"}]}"#),
            Duration::ZERO,
        );
        let player_ = Volumio::new(base, Duration::from_millis(500));
        assert_eq!(player_.queue_length().unwrap(), 3);
        assert!(served
            .join()
            .unwrap()
            .starts_with("GET /api/v1/getQueue HTTP/1.1"));
        let (base, served) = player(Some(r#"{"error":"busy"}"#), Duration::ZERO);
        assert!(Volumio::new(base, Duration::from_millis(500))
            .queue_length()
            .is_err());
        served.join().unwrap();
    }

    #[test]
    fn a_command_the_player_is_slow_to_answer_has_still_been_sent() {
        let (base, served) = player(None, Duration::from_millis(400));
        let player = Volumio::new(base, Duration::from_millis(100));
        assert!(player.transport(Transport::Toggle).is_ok());
        assert!(served.join().unwrap().contains("cmd=toggle"));
    }

    #[test]
    fn a_state_the_player_is_slow_to_give_is_an_error_and_so_is_no_player_at_all() {
        let (base, served) = player(None, Duration::from_millis(400));
        assert!(Volumio::new(base, Duration::from_millis(100))
            .state()
            .is_err());
        served.join().unwrap();
        // Nothing listens here: the port was bound and let go.
        let free = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let gone = Volumio::new(format!("http://{free}"), Duration::from_millis(100));
        assert!(gone.transport(Transport::Next).is_err());
        assert!(gone.state().is_err());
    }
}
