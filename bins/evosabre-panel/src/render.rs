//! Every screen the panel draws, written out as images for review: one
//! binary PGM a screen, 256 x 64, the sixteen greys spread over eight bits.
//! The same code draws the glass, so what is in the files is what a unit
//! shows.

use crate::config::Config;
use crate::frame::{self, Frame};
use crate::greeting;
use crate::layout::{self, Playing};
use crate::unifont::Unifont;
use audiophonics_base::clock::LocalTime;
use audiophonics_player::State;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const ADDRESS: &str = "192.168.1.42";

fn playing() -> State {
    State {
        status: "play".into(),
        title: "So Far Away".into(),
        artist: "Dire Straits".into(),
        album: "Brothers In Arms".into(),
        volume: Some(45),
        samplerate: "96 kHz".into(),
        bitdepth: "24 bit".into(),
        seek: Some(112_000),
        duration: Some(312),
        ..State::default()
    }
}

fn pgm(frame: &Frame) -> Vec<u8> {
    let mut out = format!("P5\n{} {}\n255\n", frame::WIDTH, frame::HEIGHT).into_bytes();
    for y in 0..frame::HEIGHT {
        for x in 0..frame::WIDTH {
            out.push(frame.get(x, y) * 17);
        }
    }
    out
}

/// Draw every screen into `dir`. Returns the files written.
pub fn all(dir: &Path, cfg: &Config) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut font = Unifont::new(&cfg.unifont_path);
    let t0 = Instant::now();
    let mut shots: Vec<(&str, Frame)> = Vec::new();

    let (frame, _) = greeting::load(cfg);
    shots.push(("greeting", frame));
    shots.push(("greeting-text", greeting::text(&cfg.greeting_text)));
    shots.push(("farewell", greeting::text(&cfg.farewell_text)));
    shots.push(("restart", greeting::text(&cfg.restart_text)));

    let mut paused = playing();
    paused.status = "pause".into();
    paused.mute = true;
    paused.repeat = true;
    paused.random = true;
    paused.seek = Some(201_000);

    let mut long = playing();
    long.title = "Brothers In Arms (2005 Remaster)".into();
    long.artist = "Dire Straits with the National Philharmonic".into();
    long.album = "Brothers In Arms (West Germany 'Solid' CD FLAC EX) (1985)".into();
    long.samplerate = "192 kHz".into();
    long.duration = Some(1234);
    long.seek = Some(1_000_000);

    let mut radio = playing();
    radio.title = "BBC Radio 6 Music".into();
    radio.artist = String::new();
    radio.album = String::new();
    radio.samplerate = "48 kHz".into();
    radio.bitdepth = "16 bit".into();
    radio.seek = Some(1230);
    radio.duration = Some(0);

    let mut scripts = playing();
    scripts.title = "\u{6771}\u{4EAC}\u{306E}\u{591C} \u{2014} Tokyo Nights".into();
    scripts.artist = "\u{5742}\u{672C}\u{9F8D}\u{4E00}".into();
    scripts.album = "\u{97F3}\u{697D}\u{56F3}\u{9451}".into();

    let mut cyrillic = playing();
    cyrillic.title = "\u{41B}\u{435}\u{431}\u{435}\u{434}\u{438}\u{43D}\u{43E}\u{435} \u{43E}\u{437}\u{435}\u{440}\u{43E}".into();
    cyrillic.artist =
        "\u{427}\u{430}\u{439}\u{43A}\u{43E}\u{432}\u{441}\u{43A}\u{438}\u{439}".into();
    cyrillic.album = "Swan Lake".into();

    let stopped = State {
        status: "stop".into(),
        volume: Some(0),
        ..State::default()
    };

    for (name, state, after) in [
        ("playing", &playing(), Duration::ZERO),
        ("paused-muted-repeat-shuffle", &paused, Duration::ZERO),
        ("long", &long, Duration::from_millis(3200)),
        ("radio", &radio, Duration::ZERO),
        ("scripts", &scripts, Duration::ZERO),
        ("cyrillic", &cyrillic, Duration::ZERO),
        ("stopped", &stopped, Duration::ZERO),
        ("no-player", &State::default(), Duration::ZERO),
    ] {
        let mut screen = Playing::default();
        let mut frame = Frame::new();
        screen.render(&mut frame, state, ADDRESS, t0, &mut font);
        if !after.is_zero() {
            frame = Frame::new();
            screen.render(&mut frame, state, ADDRESS, t0 + after, &mut font);
        }
        shots.push((name, frame));
    }

    let mut frame = Frame::new();
    let wednesday = LocalTime {
        year: 2026,
        month: 9,
        day: 16,
        hour: 14,
        minute: 7,
        weekday: 3,
    };
    layout::clock(
        &mut frame,
        &stopped,
        ADDRESS,
        &wednesday,
        crate::config::Unlit::Faint.level(),
    );
    shots.push(("clock", frame));

    let mut written = Vec::new();
    for (name, frame) in shots {
        let path = dir.join(format!("{name}.pgm"));
        std::fs::write(&path, pgm(&frame)).map_err(|e| format!("{}: {e}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_screen_is_written_as_a_panel_sized_image() {
        let dir = std::env::temp_dir().join(format!("evosabre-render-{}", std::process::id()));
        let cfg = Config {
            logo_path: "/nonexistent/logo.pgm".into(),
            unifont_path: "/nonexistent/unifont.hex".into(),
            ..Config::default()
        };
        let files = all(&dir, &cfg).unwrap();
        assert_eq!(files.len(), 13);
        for file in &files {
            let bytes = std::fs::read(file).unwrap();
            assert!(
                bytes.starts_with(b"P5\n256 64\n255\n"),
                "{}",
                file.display()
            );
            assert_eq!(bytes.len(), 14 + 256 * 64);
            assert!(
                bytes[14..].iter().any(|&b| b != 0),
                "{} is dark",
                file.display()
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The plugin ships the logo and the font this program draws with: both
    /// must be what it can read.
    #[test]
    fn the_logo_and_the_font_the_plugin_ships_are_usable() {
        let assets = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../plugins/evo_sabre/assets"
        );
        let logo = std::fs::read(format!("{assets}/logo.pgm")).unwrap();
        let frame = greeting::parse_pgm(&logo).unwrap();
        let greys: std::collections::BTreeSet<u8> = (0..frame::HEIGHT)
            .flat_map(|y| (0..frame::WIDTH).map(move |x| (x, y)))
            .map(|(x, y)| frame.get(x, y))
            .collect();
        assert!(
            greys.len() > 8,
            "the mark's soft edges use the panel's greys"
        );

        let mut font = Unifont::new(format!("{assets}/unifont.hex"));
        assert!(font.usable(), "{:?}", font.take_problem());
        assert_eq!(font.glyph('A').unwrap().width, 8);
        assert_eq!(font.glyph('\u{4E2D}').unwrap().width, 16);
        assert_eq!(font.glyph('\u{0416}').unwrap().width, 8, "Cyrillic");
        assert_eq!(font.glyph('\u{D55C}').unwrap().width, 16, "Hangul");
    }
}
