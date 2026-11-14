//! A minimal HTTP GET over TCP.
//!
//! A daemon talks to the player on loopback and to nothing else. Volumio
//! answers such requests whole, without chunked encoding, so the reply is
//! everything after the blank line.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Split "http://host:port/path" into (host:port, path).
fn split_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("http://")?;
    match rest.find('/') {
        Some(i) => Some((rest[..i].to_string(), rest[i..].to_string())),
        None => Some((rest.to_string(), "/".to_string())),
    }
}

/// Send a GET. The stream comes back with the request on its way and the
/// reply still to be read.
pub fn send(url: &str, timeout: Duration) -> std::io::Result<TcpStream> {
    let (authority, path) = split_url(url).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "not an http:// url")
    })?;

    let mut stream = TcpStream::connect(&authority)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;

    let head = format!(
        "GET {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\nAccept: application/json\r\n\r\n"
    );
    stream.write_all(head.as_bytes())?;
    stream.flush()?;
    Ok(stream)
}

/// The body of the reply to a request sent.
pub fn reply(mut stream: TcpStream) -> std::io::Result<String> {
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    let text = String::from_utf8_lossy(&raw);

    match text.find("\r\n\r\n") {
        Some(i) => Ok(text[i + 4..].to_string()),
        None => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "no header terminator in response",
        )),
    }
}

/// The body of the reply to a GET.
pub fn get(url: &str, timeout: Duration) -> std::io::Result<String> {
    reply(send(url, timeout)?)
}

/// True for the error a reply ends in when it did not come in time.
pub fn timed_out(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

#[cfg(test)]
mod tests {
    use super::split_url;

    #[test]
    fn splits_host_and_path() {
        assert_eq!(
            split_url("http://localhost:3000/api/v1/getState"),
            Some(("localhost:3000".into(), "/api/v1/getState".into()))
        );
    }

    #[test]
    fn defaults_an_empty_path_to_root() {
        assert_eq!(
            split_url("http://127.0.0.1:3000"),
            Some(("127.0.0.1:3000".into(), "/".into()))
        );
    }

    #[test]
    fn rejects_a_non_http_url() {
        assert_eq!(split_url("https://example.com/x"), None);
    }
}
