//! What each screen shows, and where.
//!
//! The first page is the cover with two bars laid over it: a narrow one
//! along the top with what the player is doing, the unit's address, the
//! time, repeat, shuffle and the volume; and one along the bottom with the
//! track, under a thin line that fills as the track plays. The second page
//! keeps the track at its top and lists what is known of the stream. While
//! the DAC takes its other input there is nothing to say of a track, and
//! the screen says which input it is. A mark with a word under it is shown
//! while the player starts and when it leaves.

use crate::canvas::{Area, Canvas, Rgb, BLACK, HEIGHT, WHITE, WIDTH};
use crate::config::Place;
use crate::gauge;
use crate::icons;
use crate::mark::Mark;
use crate::text::{plain, Faces};
use crate::ticker::Ticker;
use audiophonics_meters::Bar;
use audiophonics_player::State;
use std::time::Instant;

/// The colour of the line that fills as a track plays.
pub const ACCENT: Rgb = [0xA7, 0x4A, 0x0C];
/// What is said quietly: a label, a mark that is off.
pub(crate) const QUIET: Rgb = [150, 150, 150];
/// How strongly black is laid over the cover under a bar: seven tenths.
pub(crate) const BAR_VEIL: u8 = 179;
/// How strongly a mark that is off is drawn: three tenths.
const OFF: u8 = 77;

const BAR: Area = Area::new(0, 0, WIDTH, 18);
const BAR_TEXT: u32 = 14;
const BAR_BASELINE: i32 = 14;
const ICON_TOP: i32 = 4;
const STATE_LEFT: i32 = 4;
const ADDRESS_LEFT: i32 = 21;
const CLOCK_LEFT: i32 = 165;
const REPEAT_LEFT: i32 = 226;
const SHUFFLE_LEFT: i32 = 242;
const SPEAKER_LEFT: i32 = 260;
const VOLUME_RIGHT: i32 = 316;

const PROGRESS: Area = Area::new(0, 207, WIDTH, 3);
const BAND: Area = Area::new(0, 210, WIDTH, 30);
const TRACK_TEXT: u32 = 25;
const TRACK_BASELINE: i32 = 233;
const TRACK_LEFT: i32 = 6;
/// What parts a line from itself when it comes round again.
const TRACK_GAP: u32 = 60;

/// The second page is the first moved up until its band is at the top,
/// with a row beneath for each thing known.
pub const DETAILS_BAND: Area = Area::new(0, 0, WIDTH, BAND.h);
const DETAILS_TRACK_BASELINE: i32 = TRACK_BASELINE - BAND.y as i32;
const DETAILS_RULE: Area = Area::new(0, 32, WIDTH, 2);
const DETAILS_TOP: i32 = 40;
const DETAILS_PITCH: i32 = 28;
const DETAILS_LABEL_LEFT: i32 = 8;
const DETAILS_VALUE_LEFT: i32 = 128;
const DETAILS_LABEL_TEXT: u32 = 16;
const DETAILS_VALUE_TEXT: u32 = 20;
/// A value too long for its row is drawn smaller, down to this.
const DETAILS_SMALLEST: u32 = 12;

const INPUT_TEXT: u32 = 100;
const INPUT_BASELINE: i32 = 162;

const SPLASH_TEXT: u32 = 20;
/// Between the mark and the word under it.
const SPLASH_GAP: i32 = 22;
/// The dots that step while the player is waited for: their size, how far
/// apart, how far under the word, and the colour of one that is not lit.
const WAITING_DOT: u32 = 4;
const WAITING_PITCH: i32 = 12;
const WAITING_UNDER: i32 = 14;
const WAITING_DIM: Rgb = [56, 56, 56];

/// What a screen is drawn from, besides the cover.
#[derive(Debug, Clone, Copy)]
pub struct Shown<'a> {
    pub state: &'a State,
    /// The unit's address, for whoever wants to open its page.
    pub address: &'a str,
    /// The time of day, as "14:07".
    pub clock: &'a str,
    /// The DAC's filter as its mixer names it; empty where it is not known.
    pub filter: &'a str,
    /// How many tracks the queue holds, where that is known.
    pub queue: Option<u64>,
    /// The level meters, where they are on: where they stand and what
    /// each channel's shows.
    pub meters: Option<(Place, [Bar; 2])>,
}

/// The track as one line: its title, its artist and its album, whichever
/// of them there are.
pub fn track_line(state: &State) -> String {
    [&state.title, &state.artist, &state.album]
        .iter()
        .map(|part| plain(part))
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" - ")
}

pub struct Screens {
    ticker: Ticker,
}

impl Screens {
    pub fn new(now: Instant) -> Self {
        Self {
            ticker: Ticker::new(now),
        }
    }

    /// True while a line is moving: the screen is to be drawn again soon.
    pub fn moving(&self) -> bool {
        self.ticker.running()
    }

    /// The first page, over the background a cover made.
    pub fn playing(
        &mut self,
        canvas: &mut Canvas,
        backdrop: &[Rgb],
        shown: &Shown,
        faces: &mut Faces,
        now: Instant,
    ) {
        canvas.put(0, 0, WIDTH, backdrop);
        bar(canvas, shown, faces, 255);
        if let Some((place, bars)) = &shown.meters {
            gauge::draw(canvas, *place, bars, faces);
        }
        progress(canvas, shown.state);
        canvas.veil(BAND, BLACK, BAR_VEIL);
        self.track(canvas, shown.state, faces, TRACK_BASELINE, now);
    }

    /// The second page: the track as the first page has it, at the top,
    /// and what is known of the stream. The first page slides into this
    /// one: see [`Canvas::slid`].
    pub fn details(
        &mut self,
        canvas: &mut Canvas,
        backdrop: &[Rgb],
        shown: &Shown,
        faces: &mut Faces,
        now: Instant,
    ) {
        canvas.fill(BLACK);
        // The band, with the part of the cover it lies over on the first page.
        let under = (BAND.y * WIDTH) as usize..((BAND.y + BAND.h) * WIDTH) as usize;
        if let Some(rows) = backdrop.get(under) {
            canvas.put(0, 0, WIDTH, rows);
        }
        canvas.veil(DETAILS_BAND, BLACK, BAR_VEIL);
        self.track(canvas, shown.state, faces, DETAILS_TRACK_BASELINE, now);
        canvas.fill_area(DETAILS_RULE, ACCENT);
        let state = shown.state;
        let place = match (state.position, shown.queue) {
            (Some(at), Some(of)) if of > 0 => format!("{}/{of}", at + 1),
            (Some(at), _) => (at + 1).to_string(),
            _ => String::new(),
        };
        let channels = state.channels.map(|n| n.to_string()).unwrap_or_default();
        let rows: [(&str, &str); 7] = [
            ("Sample Rate", &state.samplerate),
            ("Sample Depth", &state.bitdepth),
            ("Bit Rate", &state.bitrate),
            ("Channels", &channels),
            ("Track Type", &state.track_type),
            ("Playlist", &place),
            ("DAC", shown.filter),
        ];
        for (i, (label, value)) in rows.iter().enumerate() {
            let baseline = DETAILS_TOP + DETAILS_PITCH * i as i32 + DETAILS_VALUE_TEXT as i32;
            faces.draw(
                canvas,
                label,
                DETAILS_LABEL_LEFT,
                baseline,
                DETAILS_LABEL_TEXT,
                QUIET,
            );
            let value = plain(value);
            let value = if value.trim().is_empty() {
                "-"
            } else {
                value.trim()
            };
            let room = WIDTH as i32 - DETAILS_VALUE_LEFT - DETAILS_LABEL_LEFT;
            let size = fitting(faces, value, room, DETAILS_VALUE_TEXT, DETAILS_SMALLEST);
            faces.draw(canvas, value, DETAILS_VALUE_LEFT, baseline, size, WHITE);
        }
    }

    /// The track's line with its baseline on a row, running where it is
    /// wider than the glass.
    fn track(
        &mut self,
        canvas: &mut Canvas,
        state: &State,
        faces: &mut Faces,
        baseline: i32,
        now: Instant,
    ) {
        let line = track_line(state);
        let width = faces.width(&line, TRACK_TEXT).max(0) as u32;
        let room = WIDTH - 2 * TRACK_LEFT as u32;
        self.ticker.set(&line, width, TRACK_GAP, room, now);
        let at = TRACK_LEFT - self.ticker.offset(now) as i32;
        faces.draw(canvas, &line, at, baseline, TRACK_TEXT, WHITE);
        if self.ticker.running() {
            let again = at + self.ticker.lap() as i32;
            if again < WIDTH as i32 {
                faces.draw(canvas, &line, again, baseline, TRACK_TEXT, WHITE);
            }
        }
    }
}

/// The largest size, from `size` down to `smallest`, at which a line takes
/// no more than `room`.
fn fitting(faces: &mut Faces, text: &str, room: i32, size: u32, smallest: u32) -> u32 {
    (smallest..=size)
        .rev()
        .find(|&s| faces.width(text, s) <= room)
        .unwrap_or(smallest)
}

/// The top bar, laid over whatever is there. `strength` is how strongly its
/// marks are drawn: all of it, or half while the DAC takes another input.
fn bar(canvas: &mut Canvas, shown: &Shown, faces: &mut Faces, strength: u8) {
    canvas.veil(BAR, BLACK, BAR_VEIL);
    let state = shown.state;
    let ink = [strength; 3];
    let scaled = |cover: u8| (u32::from(cover) * u32::from(strength) / 255) as u8;
    let doing = match state.status.as_str() {
        "play" => &icons::PLAY,
        "pause" => &icons::PAUSE,
        _ => &icons::STOP,
    };
    doing.draw(canvas, STATE_LEFT, ICON_TOP, WHITE, strength);
    faces.draw(
        canvas,
        shown.address,
        ADDRESS_LEFT,
        BAR_BASELINE,
        BAR_TEXT,
        ink,
    );
    faces.draw(canvas, shown.clock, CLOCK_LEFT, BAR_BASELINE, BAR_TEXT, ink);
    let repeat = if state.repeat_single {
        &icons::REPEAT_ONE
    } else {
        &icons::REPEAT
    };
    let on = |is: bool| scaled(if is { 255 } else { OFF });
    repeat.draw(canvas, REPEAT_LEFT, ICON_TOP, WHITE, on(state.repeats()));
    icons::SHUFFLE.draw(canvas, SHUFFLE_LEFT, ICON_TOP, WHITE, on(state.random));
    icons::speaker(canvas, SPEAKER_LEFT, ICON_TOP, state.volume, ink);
    if let Some(volume) = state.volume {
        let text = volume.to_string();
        let left = VOLUME_RIGHT - faces.width(&text, BAR_TEXT);
        faces.draw(canvas, &text, left, BAR_BASELINE, BAR_TEXT, ink);
    }
}

/// The line that fills as the track plays; nothing for a stream, which has
/// no length.
fn progress(canvas: &mut Canvas, state: &State) {
    let (Some(seek_ms), Some(length_s)) = (state.seek, state.duration) else {
        return;
    };
    if length_s == 0 {
        return;
    }
    let done = (u128::from(seek_ms) * u128::from(WIDTH) / (u128::from(length_s) * 1000))
        .min(u128::from(WIDTH)) as u32;
    canvas.fill_area(Area::new(PROGRESS.x, PROGRESS.y, done, PROGRESS.h), ACCENT);
}

/// The screen while the DAC takes an input other than the player: the
/// input's name, large, under a bar at half strength.
pub fn input(canvas: &mut Canvas, name: &str, shown: &Shown, faces: &mut Faces) {
    canvas.fill(BLACK);
    bar(canvas, shown, faces, 128);
    let name = plain(name);
    let size = fitting(faces, &name, WIDTH as i32 - 16, INPUT_TEXT, 24);
    let left = (WIDTH as i32 - faces.width(&name, size)) / 2;
    faces.draw(canvas, &name, left, INPUT_BASELINE, size, WHITE);
}

/// A mark with a word under it, in the middle of a black screen: the
/// player starting, getting ready, shutting down, restarting. Without a
/// mark the word stands alone. Returns the row the word's baseline is on.
pub fn splash(canvas: &mut Canvas, mark: Option<&Mark>, word: &str, faces: &mut Faces) -> i32 {
    canvas.fill(BLACK);
    let word = plain(word);
    let centre = WIDTH as i32 / 2;
    let left = centre - faces.width(&word, SPLASH_TEXT) / 2;
    match mark {
        Some(mark) => {
            let block = mark.height as i32 + SPLASH_GAP + SPLASH_TEXT as i32;
            let top = (HEIGHT as i32 - block) / 2;
            mark.draw(canvas, centre, top);
            let baseline = top + mark.height as i32 + SPLASH_GAP + SPLASH_TEXT as i32 * 3 / 4;
            faces.draw(canvas, &word, left, baseline, SPLASH_TEXT, QUIET);
            baseline
        }
        None => {
            let baseline = HEIGHT as i32 / 2 + SPLASH_TEXT as i32 / 3;
            faces.draw(canvas, &word, left, baseline, SPLASH_TEXT, WHITE);
            baseline
        }
    }
}

/// Three dots under a splash's word, one of them lit, the next at each
/// step: a sign that the unit is alive while the player is waited for.
pub fn waiting(canvas: &mut Canvas, baseline: i32, step: u32) {
    let first = WIDTH as i32 / 2 - WAITING_PITCH - WAITING_DOT as i32 / 2;
    for dot in 0..3u32 {
        let x = first + dot as i32 * WAITING_PITCH;
        let colour = if dot == step % 3 { QUIET } else { WAITING_DIM };
        canvas.fill_area(
            Area::new(
                x as u32,
                (baseline + WAITING_UNDER) as u32,
                WAITING_DOT,
                WAITING_DOT,
            ),
            colour,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art;
    use crate::text::tests::assets;
    use std::time::Duration;

    fn playing() -> State {
        State {
            status: "play".into(),
            title: "So Far Away".into(),
            artist: "Dire Straits".into(),
            album: "Brothers In Arms".into(),
            volume: Some(45),
            samplerate: "96 kHz".into(),
            bitdepth: "24 bit".into(),
            seek: Some(156_000),
            duration: Some(312),
            position: Some(2),
            track_type: "flac".into(),
            channels: Some(2),
            ..State::default()
        }
    }

    fn shown(state: &State) -> Shown<'_> {
        Shown {
            state,
            address: "192.168.1.42",
            clock: "14:07",
            filter: "brick wall",
            queue: Some(9),
            meters: None,
        }
    }

    /// How many pixels of an area are not the colour given.
    fn other_than(canvas: &Canvas, area: Area, colour: Rgb) -> usize {
        let mut n = 0;
        for y in area.y..area.y + area.h {
            for x in area.x..area.x + area.w {
                if canvas.get(x as i32, y as i32) != colour {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn the_track_is_one_line_of_what_there_is() {
        assert_eq!(
            track_line(&playing()),
            "So Far Away - Dire Straits - Brothers In Arms"
        );
        let radio = State {
            title: " BBC Radio 6 Music\n".into(),
            ..State::default()
        };
        assert_eq!(track_line(&radio), "BBC Radio 6 Music");
        assert_eq!(track_line(&State::default()), "");
    }

    #[test]
    fn the_first_page_is_the_cover_under_two_bars_and_a_line_half_filled() {
        let t0 = Instant::now();
        let mut faces = Faces::open(&assets());
        let mut screens = Screens::new(t0);
        let mut canvas = Canvas::new();
        let cover = vec![[200, 200, 200]; (WIDTH * HEIGHT) as usize];
        let state = playing();
        screens.playing(&mut canvas, &cover, &shown(&state), &mut faces, t0);
        // The cover shows in the middle, untouched.
        assert_eq!(canvas.get(160, 100), [200, 200, 200]);
        // Under the bars it is veiled: three tenths of it are left.
        assert_eq!(canvas.get(150, 2), [60, 60, 60]);
        assert_eq!(canvas.get(318, 238), [60, 60, 60]);
        // The bars carry something.
        assert!(other_than(&canvas, BAR, [60, 60, 60]) > 300);
        assert!(other_than(&canvas, BAND, [60, 60, 60]) > 800);
        // The play mark is at the left of the top bar.
        assert_eq!(canvas.get(4, 4), WHITE);
        // Half the track is played: half the line is filled.
        assert_eq!(canvas.get(159, 208), ACCENT);
        assert_eq!(canvas.get(160, 208), [200, 200, 200]);
        // Title, artist and album at this size are wider than the glass.
        assert!(screens.moving());
        let short = State {
            title: "So Far Away".into(),
            ..playing()
        };
        let short = State {
            artist: String::new(),
            album: String::new(),
            ..short
        };
        screens.playing(&mut canvas, &cover, &shown(&short), &mut faces, t0);
        assert!(!screens.moving(), "a title alone fits");
    }

    #[test]
    fn a_stream_has_no_line_and_a_track_past_its_length_a_full_one() {
        let mut canvas = Canvas::new();
        let mut radio = playing();
        radio.duration = Some(0);
        progress(&mut canvas, &radio);
        radio.duration = None;
        progress(&mut canvas, &radio);
        assert_eq!(canvas, Canvas::new());
        let mut over = playing();
        over.seek = Some(999_000);
        progress(&mut canvas, &over);
        assert_eq!(canvas.get(319, 207), ACCENT);
        assert_eq!(canvas.get(319, 209), ACCENT);
        assert_eq!(canvas.get(319, 210), BLACK);
    }

    #[test]
    fn a_line_wider_than_the_glass_runs_and_comes_round() {
        let t0 = Instant::now();
        let mut faces = Faces::open(&assets());
        let mut screens = Screens::new(t0);
        let mut state = playing();
        state.album = "Brothers In Arms (West Germany 'Solid' CD) (1985)".into();
        let cover = art::none();
        let mut held = Canvas::new();
        screens.playing(&mut held, &cover, &shown(&state), &mut faces, t0);
        assert!(screens.moving());
        let mut same = Canvas::new();
        screens.playing(
            &mut same,
            &cover,
            &shown(&state),
            &mut faces,
            t0 + Duration::from_secs(1),
        );
        assert_eq!(held, same, "held at its start");
        let mut run = Canvas::new();
        screens.playing(
            &mut run,
            &cover,
            &shown(&state),
            &mut faces,
            t0 + Duration::from_secs(4),
        );
        assert_ne!(held, run);
        // Only the band moved.
        let above = Area::new(0, 0, WIDTH, 210);
        for y in above.y..above.h {
            for x in 0..WIDTH {
                assert_eq!(held.get(x as i32, y as i32), run.get(x as i32, y as i32));
            }
        }
    }

    #[test]
    fn marks_that_are_off_are_faint_and_the_speaker_is_crossed_at_nought() {
        let t0 = Instant::now();
        let mut faces = Faces::open(&assets());
        let mut screens = Screens::new(t0);
        let cover = art::none();
        let draw = |screens: &mut Screens, faces: &mut Faces, state: &State| {
            let mut canvas = Canvas::new();
            screens.playing(&mut canvas, &cover, &shown(state), faces, t0);
            canvas
        };
        let off = draw(&mut screens, &mut faces, &playing());
        // Repeat's arrow, off: three tenths of white.
        assert_eq!(off.get(REPEAT_LEFT + 4, ICON_TOP), [77, 77, 77]);
        let mut state = playing();
        state.repeat = true;
        state.random = true;
        state.volume = Some(0);
        state.status = "pause".into();
        let on = draw(&mut screens, &mut faces, &state);
        assert_eq!(on.get(REPEAT_LEFT + 4, ICON_TOP), WHITE);
        assert_eq!(on.get(SHUFFLE_LEFT, ICON_TOP + 1), WHITE);
        // Paused: two bars with a gap.
        assert_eq!(on.get(STATE_LEFT + 1, ICON_TOP), WHITE);
        assert_eq!(on.get(STATE_LEFT + 4, ICON_TOP), BLACK);
        // The cross of a speaker at nought.
        assert_eq!(on.get(SPEAKER_LEFT + 8, ICON_TOP + 2), WHITE);
        assert_eq!(off.get(SPEAKER_LEFT + 8, ICON_TOP + 2), BLACK);
    }

    #[test]
    fn the_second_page_lists_what_is_known_and_a_dash_for_what_is_not() {
        let t0 = Instant::now();
        let mut faces = Faces::open(&assets());
        let mut screens = Screens::new(t0);
        let mut canvas = Canvas::new();
        let state = playing();
        let cover = vec![[200, 200, 200]; (WIDTH * HEIGHT) as usize];
        screens.details(&mut canvas, &cover, &shown(&state), &mut faces, t0);
        assert_eq!(canvas.get(0, 32), ACCENT);
        // Its band is the first page's, moved to the top: the first page
        // slid all the way is this page.
        let mut first = Canvas::new();
        screens.playing(&mut first, &cover, &shown(&state), &mut faces, t0);
        assert_eq!(first.slid(&canvas, BAND.y, DETAILS_BAND.h), canvas);
        // Seven rows, each with a label and a value.
        for row in 0..7 {
            let top = (DETAILS_TOP + DETAILS_PITCH * row) as u32;
            let labels = Area::new(0, top, 120, DETAILS_PITCH as u32);
            let values = Area::new(128, top, 192, DETAILS_PITCH as u32);
            assert!(other_than(&canvas, labels, BLACK) > 40, "label {row}");
            assert!(other_than(&canvas, values, BLACK) > 8, "value {row}");
        }
        // A long filter's name is made to fit.
        let long = Shown {
            filter: "corrected minimum phase fast",
            ..shown(&state)
        };
        let mut canvas = Canvas::new();
        screens.details(&mut canvas, &cover, &long, &mut faces, t0);
        assert_eq!(other_than(&canvas, Area::new(316, 200, 4, 40), BLACK), 0);
        assert!(fitting(&mut faces, "corrected minimum phase fast", 184, 20, 12) < 20);
        assert_eq!(fitting(&mut faces, "flac", 184, 20, 12), 20);
    }

    #[test]
    fn another_input_is_named_large_under_a_bar_at_half_strength() {
        let mut faces = Faces::open(&assets());
        let mut canvas = Canvas::new();
        let state = playing();
        input(&mut canvas, "SPDIF", &shown(&state), &mut faces);
        assert_eq!(canvas.get(4, 4), [128, 128, 128], "the play mark, at half");
        let name = Area::new(8, 80, 304, 90);
        assert!(other_than(&canvas, name, BLACK) > 4000);
        // The name is centred: as much to its left as to its right.
        let first = (0..WIDTH as i32).find(|&x| (80..170).any(|y| canvas.get(x, y) != BLACK));
        let last = (0..WIDTH as i32)
            .rev()
            .find(|&x| (80..170).any(|y| canvas.get(x, y) != BLACK));
        let (first, last) = (first.unwrap(), last.unwrap());
        assert!(
            (first - (WIDTH as i32 - 1 - last)).abs() <= 12,
            "{first} {last}"
        );
    }

    #[test]
    fn the_mark_stands_over_its_word_and_the_word_alone_without_it() {
        let mut faces = Faces::open(&assets());
        let mark = crate::mark::tests::sample();
        let mut canvas = Canvas::new();
        splash(&mut canvas, Some(&mark), "Starting", &mut faces);
        assert_eq!(canvas.get(158, 98), WHITE, "the mark, above the middle");
        let word = Area::new(100, 118, 120, 30);
        assert!(other_than(&canvas, word, BLACK) > 150);
        let mut alone = Canvas::new();
        splash(&mut alone, None, "Starting", &mut faces);
        assert_eq!(other_than(&alone, Area::new(0, 0, WIDTH, 100), BLACK), 0);
        assert!(other_than(&alone, Area::new(100, 105, 120, 30), BLACK) > 150);
    }
}
