//! Every screen the panel draws, written out as images for review: one
//! binary PPM a screen, 320 x 240, in the colours the panel has. The same
//! code draws the glass, so what is in the files is what a unit shows.

use crate::art;
use crate::canvas::{rgb888, Canvas, Frame, HEIGHT, WIDTH};
use crate::config::Place;
use crate::mark::Mark;
use crate::screens::{self, Screens, Shown};
use crate::text::Faces;
use audiophonics_meters::{self as meters, Bar};
use audiophonics_player::State;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const ADDRESS: &str = "192.168.1.42";
const CLOCK: &str = "14:07";

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
        channels: Some(2),
        track_type: "flac".into(),
        position: Some(2),
        ..State::default()
    }
}

fn show(state: &State) -> Shown<'_> {
    Shown {
        state,
        address: ADDRESS,
        clock: CLOCK,
        filter: "brick wall",
        queue: Some(9),
        meters: None,
    }
}

/// A canvas as the panel shows it: brought to the panel's sixteen bits and
/// back, as a binary PPM.
fn ppm(canvas: &Canvas) -> Vec<u8> {
    let frame = Frame::from(canvas);
    let mut out = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            out.extend_from_slice(&rgb888(frame.get(x, y)));
        }
    }
    out
}

/// Draw every screen into `dir` with the faces and the mark in `assets`.
/// Returns the files written.
pub fn all(dir: &Path, assets: &Path) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut faces = Faces::open(assets);
    let mark = Mark::open(assets).ok();
    let t0 = Instant::now();
    let mut shots: Vec<(&str, Canvas)> = Vec::new();

    for (name, word) in [
        ("starting", "Starting"),
        ("shutting-down", "Shutting down"),
        ("restarting", "Restarting"),
    ] {
        let mut canvas = Canvas::new();
        screens::splash(&mut canvas, mark.as_ref(), word, &mut faces);
        shots.push((name, canvas));
    }
    // The player answers and has not finished starting: the word for it,
    // and the dots that step while it is waited for.
    let mut canvas = Canvas::new();
    let baseline = screens::splash(&mut canvas, mark.as_ref(), "Getting ready", &mut faces);
    screens::waiting(&mut canvas, baseline, 1);
    shots.push(("getting-ready", canvas));

    let square = art::backdrop(&art::sample(600, 600));
    let wide = art::backdrop(&art::sample(900, 400));
    let tall = art::backdrop(&art::sample(300, 500));
    let small = art::backdrop(&art::sample(96, 96));
    let none = art::none();

    let mut paused = playing();
    paused.status = "pause".into();
    paused.volume = Some(0);
    paused.repeat = true;
    paused.random = true;
    paused.seek = Some(201_000);

    let mut long = playing();
    long.title = "Brothers In Arms (2005 Remaster)".into();
    long.artist = "Dire Straits with the National Philharmonic".into();
    long.album = "Brothers In Arms (West Germany 'Solid' CD FLAC EX) (1985)".into();
    long.samplerate = "192 kHz".into();
    long.volume = Some(100);
    long.repeat_single = true;

    let mut radio = playing();
    radio.title = "BBC Radio 6 Music".into();
    radio.artist = String::new();
    radio.album = String::new();
    radio.samplerate = "48 kHz".into();
    radio.bitdepth = "16 bit".into();
    radio.bitrate = "320 Kbps".into();
    radio.track_type = "webradio".into();
    radio.seek = Some(1230);
    radio.duration = Some(0);
    radio.volume = Some(20);

    let mut scripts = playing();
    scripts.title = "\u{6771}\u{4EAC}\u{306E}\u{591C} \u{2014} Tokyo Nights".into();
    scripts.artist = "\u{5742}\u{672C}\u{9F8D}\u{4E00}".into();
    scripts.album = String::new();

    let mut cyrillic = playing();
    cyrillic.title = "\u{41B}\u{435}\u{431}\u{435}\u{434}\u{438}\u{43D}\u{43E}\u{435} \u{43E}\u{437}\u{435}\u{440}\u{43E}".into();
    cyrillic.artist =
        "\u{427}\u{430}\u{439}\u{43A}\u{43E}\u{432}\u{441}\u{43A}\u{438}\u{439}".into();
    cyrillic.album = String::new();

    let stopped = State {
        status: "stop".into(),
        volume: Some(45),
        ..State::default()
    };

    for (name, state, cover, after) in [
        ("playing", &playing(), &square, Duration::ZERO),
        (
            "paused-silent-repeat-shuffle",
            &paused,
            &square,
            Duration::ZERO,
        ),
        ("long", &long, &wide, Duration::from_millis(5200)),
        ("radio", &radio, &none, Duration::ZERO),
        ("tall-cover", &playing(), &tall, Duration::ZERO),
        ("small-cover", &playing(), &small, Duration::ZERO),
        ("scripts", &scripts, &square, Duration::ZERO),
        ("cyrillic", &cyrillic, &square, Duration::ZERO),
        ("stopped", &stopped, &none, Duration::ZERO),
    ] {
        let mut screens = Screens::new(t0);
        let mut canvas = Canvas::new();
        screens.playing(&mut canvas, cover, &show(state), &mut faces, t0);
        if !after.is_zero() {
            screens.playing(&mut canvas, cover, &show(state), &mut faces, t0 + after);
        }
        shots.push((name, canvas));
    }

    // The level meters, where each can stand: the left channel a little
    // louder than the right, each with its peak and the mark that holds.
    let bars = [
        Bar {
            level: meters::place(-14.0),
            peak: meters::place(-6.0),
            hold: meters::place(-2.5),
        },
        Bar {
            level: meters::place(-19.0),
            peak: meters::place(-11.0),
            hold: meters::place(-8.0),
        },
    ];
    for (name, place) in [
        ("meters-right", Place::Right),
        ("meters-left", Place::Left),
        ("meters-sides", Place::Sides),
        ("meters-lying", Place::Over),
    ] {
        let state = playing();
        let shown = Shown {
            meters: Some((place, bars)),
            ..show(&state)
        };
        let mut screens = Screens::new(t0);
        let mut canvas = Canvas::new();
        screens.playing(&mut canvas, &square, &shown, &mut faces, t0);
        shots.push((name, canvas));
    }

    // The second page, and the first on its way to it.
    let mut screens = Screens::new(t0);
    let state = playing();
    let mut first = Canvas::new();
    screens.playing(&mut first, &square, &show(&state), &mut faces, t0);
    let mut second = Canvas::new();
    screens.details(&mut second, &square, &show(&state), &mut faces, t0);
    let band = screens::DETAILS_BAND.h;
    shots.push(("sliding", first.slid(&second, (HEIGHT - band) / 2, band)));
    shots.push(("details", second));
    let long_filter = Shown {
        filter: "corrected minimum phase fast",
        ..show(&radio)
    };
    let mut canvas = Canvas::new();
    Screens::new(t0).details(&mut canvas, &none, &long_filter, &mut faces, t0);
    shots.push(("details-radio", canvas));

    let mut canvas = Canvas::new();
    screens::input(&mut canvas, "SPDIF", &show(&stopped), &mut faces);
    shots.push(("spdif", canvas));

    let mut written = Vec::new();
    for (name, canvas) in shots {
        let path = dir.join(format!("{name}.ppm"));
        std::fs::write(&path, ppm(&canvas)).map_err(|e| format!("{}: {e}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::tests::assets;

    #[test]
    fn every_screen_is_written_as_a_panel_sized_image() {
        let dir = std::env::temp_dir().join(format!("raspdacmini-render-{}", std::process::id()));
        let files = all(&dir, &assets()).unwrap();
        assert_eq!(files.len(), 21);
        for file in &files {
            let bytes = std::fs::read(file).unwrap();
            assert!(
                bytes.starts_with(b"P6\n320 240\n255\n"),
                "{}",
                file.display()
            );
            assert_eq!(bytes.len(), 15 + 320 * 240 * 3);
            assert!(
                bytes[15..].iter().any(|&b| b != 0),
                "{} is dark",
                file.display()
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_screens_are_drawn_without_any_asset_at_all() {
        let dir = std::env::temp_dir().join(format!("raspdacmini-bare-{}", std::process::id()));
        let files = all(&dir, Path::new("/nonexistent")).unwrap();
        assert_eq!(files.len(), 21);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
