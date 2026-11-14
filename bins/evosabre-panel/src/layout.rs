//! The screens, drawn in the panel's coordinates.
//!
//! The picture uses 255 x 63 of the 256 x 64: the last column and row are
//! spare, so the pixel orbit can move everything by one without cutting
//! anything. Grey levels carry the hierarchy: the title brightest, then the
//! artist, then the album, with the chrome quieter than any of them.

use crate::frame::Frame;
use crate::grey;
use crate::icons;
use crate::scroll::Marquee;
use crate::segments;
use crate::text::{self, Face};
use crate::unifont::Unifont;
use audiophonics_base::clock::LocalTime;
use audiophonics_player::State;
use embedded_graphics::mono_font::iso_8859_1::{FONT_5X8, FONT_6X10, FONT_7X13, FONT_9X18_BOLD};
use embedded_graphics::mono_font::MonoFont;
use std::time::{Duration, Instant};

pub const TITLE: u8 = grey::linear(15);
pub const ARTIST: u8 = grey::linear(11);
pub const ALBUM: u8 = grey::linear(8);
pub const CHROME: u8 = grey::linear(10);
pub const ICON: u8 = grey::linear(12);
pub const QUIET: u8 = grey::linear(6);
pub const HAIRLINE: u8 = grey::linear(5);
pub const BAR_TRACK: u8 = grey::linear(3);
pub const BAR_FILL: u8 = grey::linear(12);

/// The picture's width: one less than the panel, as is its height, so the
/// orbit has a spare column and row.
pub const W: u32 = 255;

const STATUS_FONT: &MonoFont = &FONT_6X10;
const TIME_FONT: &MonoFont = &FONT_5X8;
pub const HAIRLINE_Y: u32 = 10;
const TITLE_Y: i32 = 12;
/// The rows the title has: its own face fills them, the wide one sits
/// centred in them.
const TITLE_ROWS: u32 = 18;
const ARTIST_Y: i32 = 31;
const ALBUM_Y: i32 = 45;
const TIME_Y: i32 = 55;
const BAR_Y: u32 = 58;

/// A line in one of the panel's own faces, where it stands.
fn mono(f: &mut Frame, x: i32, y: i32, s: &str, font: &'static MonoFont<'static>, luma: u8) {
    let mut none = Unifont::new("");
    Face::Mono(font).draw(f, &mut none, s, y, luma, -x, W);
}

/// At most `max_chars`, the last three replaced by dots when cut.
fn cut(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars - 3).collect();
    out.push_str("...");
    out
}

pub fn mmss(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// The status row and the hairline. The format is the last track's, so the
/// clock leaves it out.
fn chrome(f: &mut Frame, s: &State, ip: &str, with_format: bool) {
    let cw = STATUS_FONT.character_size.width;

    let transport = match s.status.as_str() {
        "play" => &icons::PLAY,
        "pause" => &icons::PAUSE,
        _ => &icons::STOP,
    };
    icons::draw(f, transport, 2, 0, ICON);

    let silent = s.mute || s.volume == Some(0);
    let speaker = if silent {
        &icons::SPEAKER_OFF
    } else {
        &icons::SPEAKER
    };
    icons::draw(f, speaker, 16, 0, if silent { QUIET } else { ICON });
    let vol = match s.volume {
        Some(v) => v.to_string(),
        None => "-".into(),
    };
    mono(
        f,
        31,
        0,
        &vol,
        STATUS_FONT,
        if silent { QUIET } else { CHROME },
    );
    // Room for "100" whatever the number is, so what follows does not jump.
    let mut left_end = 31 + 3 * cw + 4;
    for (on, icon) in [(s.repeats(), &icons::REPEAT), (s.random, &icons::RANDOM)] {
        if on {
            icons::draw(f, icon, left_end, 0, CHROME);
            left_end += icon.width() + 4;
        }
    }

    let ip_w = ip.chars().count() as u32 * cw;
    let ip_x = W.saturating_sub(ip_w);
    mono(f, ip_x as i32, 0, ip, STATUS_FONT, CHROME);

    if with_format {
        let format = format!("{} {}", s.samplerate, s.bitdepth);
        let format = format.trim();
        let span = ip_x.saturating_sub(4).saturating_sub(left_end);
        let fw = format.chars().count() as u32 * cw;
        if fw <= span {
            let fx = left_end + (span - fw) / 2;
            mono(f, fx as i32, 0, format, STATUS_FONT, CHROME);
        }
    }

    for px in 0..W {
        f.set(px, HAIRLINE_Y, HAIRLINE);
    }
}

/// Elapsed at the left, length at the right, the bar between them.
fn time_row(f: &mut Frame, s: &State) {
    let (Some(seek), Some(duration)) = (s.seek, s.duration) else {
        return;
    };
    if duration == 0 {
        return;
    }
    let cw = TIME_FONT.character_size.width;
    let elapsed = mmss((seek / 1000).min(duration));
    let length = mmss(duration);
    let ew = elapsed.chars().count() as u32 * cw;
    let lw = length.chars().count() as u32 * cw;
    mono(f, 1, TIME_Y, &elapsed, TIME_FONT, ALBUM);
    mono(f, (W - lw) as i32, TIME_Y, &length, TIME_FONT, ALBUM);

    let x0 = 1 + ew + 6;
    let x1 = W - lw - 6;
    if x1 <= x0 {
        return;
    }
    let fill = x0 + ((seek / 1000).min(duration) * u64::from(x1 - x0) / duration) as u32;
    for px in x0..x1 {
        let l = if px < fill { BAR_FILL } else { BAR_TRACK };
        f.set(px, BAR_Y, l);
        f.set(px, BAR_Y + 1, l);
    }
}

fn title_or_status(s: &State) -> (String, u8) {
    if s.title.is_empty() {
        let word = match s.status.as_str() {
            "stop" => "Stopped",
            "" => "No player",
            other => other,
        };
        (word.to_string(), ALBUM)
    } else {
        (text::tidy(&s.title), TITLE)
    }
}

/// The artist and the album on one line, for when they share it.
fn joined(artist: &str, album: &str) -> String {
    match (artist.is_empty(), album.is_empty()) {
        (false, false) => format!("{artist} - {album}"),
        (false, true) => artist.to_string(),
        _ => album.to_string(),
    }
}

/// The screen shown while the player is up: title, artist, album, and the
/// time row. Title and artist scroll when they do not fit; the album is cut.
///
/// A line the panel's own faces cannot draw goes in the wide font, which is
/// taller: the title still has its rows, but the artist and the album then
/// share the one line under it, scrolling, since two wide lines do not fit
/// above the time row.
#[derive(Debug, Default)]
pub struct Playing {
    title: Marquee,
    second: Marquee,
}

impl Playing {
    pub fn render(&mut self, f: &mut Frame, s: &State, ip: &str, now: Instant, font: &mut Unifont) {
        chrome(f, s, ip, true);

        let (title, luma) = title_or_status(s);
        let face = Face::choose(&title, &FONT_9X18_BOLD, font);
        self.title.set(&title, face.width(&title, font), W, now);
        let drop = (TITLE_ROWS - face.height()) as i32 / 2;
        face.draw(
            f,
            font,
            &title,
            TITLE_Y + drop,
            luma,
            self.title.offset(now),
            W,
        );

        let artist = text::tidy(&s.artist);
        let album = text::tidy(&s.album);
        if text::latin(&artist) && text::latin(&album) {
            let face = Face::Mono(&FONT_7X13);
            self.second.set(&artist, face.width(&artist, font), W, now);
            face.draw(
                f,
                font,
                &artist,
                ARTIST_Y,
                ARTIST,
                self.second.offset(now),
                W,
            );
            let album_chars = (W / FONT_6X10.character_size.width) as usize;
            mono(f, 0, ALBUM_Y, &cut(&album, album_chars), &FONT_6X10, ALBUM);
        } else {
            let both = joined(&artist, &album);
            let face = Face::choose(&both, &FONT_7X13, font);
            self.second.set(&both, face.width(&both, font), W, now);
            face.draw(f, font, &both, ARTIST_Y, ARTIST, self.second.offset(now), W);
        }
        time_row(f, s);
    }

    /// True while a line is moving, so the caller redraws at the marquee's
    /// rate rather than the player's.
    pub fn scrolling(&self) -> bool {
        self.title.scrolling() || self.second.scrolling()
    }
}

/// The screen after a long rest: the time, large, and the date beside it,
/// under the same chrome.
pub fn clock(f: &mut Frame, s: &State, ip: &str, t: &LocalTime, unlit: u8) {
    chrome(f, s, ip, false);
    // 48 px of digits centred in the rows under the hairline.
    let top = HAIRLINE_Y + 1 + (H_BELOW - segments::DIGIT_H) / 2;
    segments::draw(f, 4, top, &t.hhmm(), ICON, unlit);
    let cw = FONT_7X13.character_size.width;
    for (i, line) in [t.weekday_name().to_string(), t.date_line()]
        .iter()
        .enumerate()
    {
        let w = line.chars().count() as u32 * cw;
        mono(
            f,
            (W - w) as i32,
            21 + 16 * i as i32,
            line,
            &FONT_7X13,
            ALBUM,
        );
    }
}

/// Rows under the hairline, to the picture's last row.
const H_BELOW: u32 = 63 - (HAIRLINE_Y + 1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Playing,
    Clock,
}

/// Which screen the moment calls for. Playing, or a rest shorter than
/// `clock_after`, keeps the track; a longer rest brings the clock. A pause
/// is a rest as a stop is: the player's play button pauses, so a paused
/// player is how most listening ends.
#[derive(Debug, Default)]
pub struct Selector {
    resting_since: Option<Instant>,
}

impl Selector {
    pub fn screen(&mut self, now: Instant, s: &State, clock_after: Duration) -> Screen {
        if s.is_playing() {
            self.resting_since = None;
            return Screen::Playing;
        }
        let since = *self.resting_since.get_or_insert(now);
        if !clock_after.is_zero() && now.saturating_duration_since(since) >= clock_after {
            Screen::Clock
        } else {
            Screen::Playing
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::frame;
    use crate::unifont::tests::sample;

    pub fn dire_straits() -> State {
        State {
            status: "play".into(),
            title: "So Far Away".into(),
            artist: "Dire Straits".into(),
            album: "Brothers In Arms".into(),
            volume: Some(45),
            mute: false,
            samplerate: "96 kHz".into(),
            bitdepth: "16 bit".into(),
            seek: Some(112_000),
            duration: Some(312),
            ..State::default()
        }
    }

    fn render(s: &State, ip: &str) -> Frame {
        let mut f = Frame::new();
        let mut none = Unifont::new("/nonexistent/unifont.hex");
        Playing::default().render(&mut f, s, ip, Instant::now(), &mut none);
        f
    }

    /// The columns of the status row that carry anything, between two
    /// bounds.
    fn lit_columns(f: &Frame, from: u32, to: u32) -> Vec<u32> {
        (from..to)
            .filter(|&x| (0..10).any(|y| f.get(x, y) > 0))
            .collect()
    }

    #[test]
    fn the_status_row_keeps_its_columns_and_the_hairline_is_full_width() {
        let f = render(&dire_straits(), "192.168.1.42");
        assert!(
            f.get(2, 4) > 0 && f.get(0, 4) == 0,
            "transport glyph starts at x=2"
        );
        assert!((0..W).all(|x| f.get(x, HAIRLINE_Y) == HAIRLINE));
        assert!(
            (244..W).any(|x| (0..10).any(|y| f.get(x, y) > 0)),
            "address flush right"
        );
        assert_eq!(mmss(312), "5:12");
        assert_eq!(mmss(5), "0:05");
        assert_eq!(cut("Brothers In Arms", 42), "Brothers In Arms");
        assert_eq!(cut(&"a".repeat(50), 42), format!("{}...", "a".repeat(39)));
    }

    #[test]
    fn the_spare_column_and_row_stay_dark_so_the_orbit_cuts_nothing() {
        for s in [dire_straits(), {
            let mut l = dire_straits();
            l.title = "x".repeat(28);
            l.artist = "y".repeat(36);
            l.album = "z".repeat(60);
            l.duration = Some(3599);
            l.seek = Some(3_599_000);
            l.repeat = true;
            l.random = true;
            l
        }] {
            let f = render(&s, "192.168.100.100");
            assert!(
                (0..frame::HEIGHT).all(|y| f.get(255, y) == 0),
                "column 255 lit"
            );
            assert!((0..frame::WIDTH).all(|x| f.get(x, 63) == 0), "row 63 lit");
        }
    }

    #[test]
    fn muted_swaps_the_speaker_and_quietens_the_number() {
        let mut s = dire_straits();
        s.mute = true;
        let f = render(&s, "192.168.1.42");
        let fr = &f;
        let brightest = (31..49)
            .flat_map(|x| (0..10).map(move |y| fr.get(x, y)))
            .max()
            .unwrap();
        assert_eq!(brightest, QUIET);
    }

    #[test]
    fn repeat_and_shuffle_stand_after_the_volume_only_while_they_are_on() {
        let plain = render(&dire_straits(), "10.0.0.1");
        assert!(
            lit_columns(&plain, 53, 75).is_empty(),
            "nothing after the volume"
        );
        let mut s = dire_straits();
        s.repeat_single = true;
        let one = render(&s, "10.0.0.1");
        assert_eq!(lit_columns(&one, 53, 75), (53..62).collect::<Vec<_>>());
        s.random = true;
        let both = render(&s, "10.0.0.1");
        let lit = lit_columns(&both, 53, 80);
        assert_eq!((*lit.first().unwrap(), *lit.last().unwrap()), (53, 74));
        assert!(!lit.contains(&63), "a gap between the two");
    }

    #[test]
    fn the_time_row_has_digits_at_both_ends_and_a_proportional_bar() {
        let f = render(&dire_straits(), "10.0.0.1");
        assert!(
            (1..21).any(|x| f.get(x, TIME_Y as u32 + 2) > 0),
            "elapsed at the left"
        );
        assert!(
            (235..W).any(|x| f.get(x, TIME_Y as u32 + 2) > 0),
            "length at the right"
        );
        let filled = (0..W).filter(|&x| f.get(x, BAR_Y) == BAR_FILL).count() as u64;
        let track = (0..W).filter(|&x| f.get(x, BAR_Y) == BAR_TRACK).count() as u64;
        assert_eq!(filled, 112 * (filled + track) / 312);
        let mut s = dire_straits();
        s.duration = Some(0);
        let f = render(&s, "10.0.0.1");
        assert!((0..W).all(|x| f.get(x, BAR_Y) == 0), "no bar for a stream");
    }

    pub fn wednesday() -> LocalTime {
        LocalTime {
            year: 2026,
            month: 9,
            day: 16,
            hour: 14,
            minute: 7,
            weekday: 3,
        }
    }

    #[test]
    fn the_clock_is_large_at_the_left_and_the_date_flush_right() {
        let mut f = Frame::new();
        let stopped = State {
            status: "stop".into(),
            volume: Some(45),
            samplerate: "96 kHz".into(),
            bitdepth: "16 bit".into(),
            ..State::default()
        };
        clock(&mut f, &stopped, "192.168.1.42", &wednesday(), grey::FAINT);
        assert!(
            (4..24).any(|x| (15..55).any(|y| f.get(x, y) == ICON)),
            "first digit"
        );
        assert!(
            (240..W).any(|x| (21..34).any(|y| f.get(x, y) == ALBUM)),
            "weekday flush right"
        );
        assert!((0..frame::HEIGHT).all(|y| f.get(255, y) == 0));
        assert!((0..frame::WIDTH).all(|x| f.get(x, 63) == 0));
        // No format on the clock: the middle of the status row is dark.
        assert!((70..150).all(|x| (0..10).all(|y| f.get(x, y) == 0)));
    }

    #[test]
    fn the_clock_comes_after_a_rest_and_leaves_on_play() {
        let t0 = Instant::now();
        let after = Duration::from_secs(60);
        let at = |secs: u64| t0 + Duration::from_secs(secs);
        let mut sel = Selector::default();
        let mut s = dire_straits();
        assert_eq!(sel.screen(t0, &s, after), Screen::Playing);
        s.status = "pause".into();
        assert_eq!(sel.screen(at(1), &s, after), Screen::Playing);
        assert_eq!(sel.screen(at(59), &s, after), Screen::Playing);
        assert_eq!(
            sel.screen(at(61), &s, after),
            Screen::Clock,
            "a pause rests"
        );
        s.status = "stop".into();
        assert_eq!(sel.screen(at(62), &s, after), Screen::Clock);
        s.status = "play".into();
        assert_eq!(sel.screen(at(63), &s, after), Screen::Playing);
        s.status = "stop".into();
        assert_eq!(sel.screen(at(64), &s, after), Screen::Playing, "restarted");
        assert_eq!(
            sel.screen(at(1000), &s, Duration::ZERO),
            Screen::Playing,
            "zero: never"
        );
    }

    #[test]
    fn a_long_title_scrolls_and_reports_it() {
        let mut s = dire_straits();
        s.title = "Brothers In Arms (2005 Remaster, Live)".into();
        let mut p = Playing::default();
        let mut none = Unifont::new("/nonexistent/unifont.hex");
        let t0 = Instant::now();
        let mut a = Frame::new();
        p.render(&mut a, &s, "10.0.0.1", t0, &mut none);
        assert!(p.scrolling());
        let mut b = Frame::new();
        p.render(
            &mut b,
            &s,
            "10.0.0.1",
            t0 + Duration::from_millis(3000),
            &mut none,
        );
        assert_ne!(a, b, "the title has moved");
    }

    #[test]
    fn a_title_beyond_latin_goes_in_the_wide_font_and_the_rest_keep_their_lines() {
        let path = sample("title");
        let mut font = Unifont::new(&path);
        let mut s = dire_straits();
        s.title = "\u{4E2D}\u{4E2D}".into();
        let mut f = Frame::new();
        Playing::default().render(&mut f, &s, "10.0.0.1", Instant::now(), &mut font);
        // The sample glyph's full row is its fourth; the title sits one
        // row down in its eighteen.
        let row = (TITLE_Y + 1 + 3) as u32;
        assert!((0..15).all(|x| f.get(x, row) == TITLE));
        assert!((16..31).all(|x| f.get(x, row) == TITLE), "the second glyph");
        assert!(
            (0..80).any(|x| (ALBUM_Y as u32..ALBUM_Y as u32 + 10).any(|y| f.get(x, y) == ALBUM)),
            "the album still has its own line"
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn an_artist_beyond_latin_shares_one_wide_line_with_the_album() {
        let path = sample("second");
        let mut font = Unifont::new(&path);
        let mut s = dire_straits();
        s.artist = "\u{4E2D}".into();
        s.album = "A".into();
        let mut f = Frame::new();
        Playing::default().render(&mut f, &s, "10.0.0.1", Instant::now(), &mut font);
        let row = (ARTIST_Y + 3) as u32;
        assert!((0..15).all(|x| f.get(x, row) == ARTIST), "the artist, wide");
        assert!(
            (0..W).all(|x| (48..TIME_Y as u32).all(|y| f.get(x, y) == 0)),
            "nothing between the wide line and the time row"
        );
        assert_eq!(joined("a", "b"), "a - b");
        assert_eq!(joined("a", ""), "a");
        assert_eq!(joined("", "b"), "b");
        assert_eq!(joined("", ""), "");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_player_that_is_down_and_one_that_is_stopped_say_so() {
        assert_eq!(title_or_status(&State::default()).0, "No player");
        let stopped = State {
            status: "stop".into(),
            ..State::default()
        };
        assert_eq!(title_or_status(&stopped), ("Stopped".to_string(), ALBUM));
        let mut curly = dire_straits();
        curly.title = "Don\u{2019}t".into();
        assert_eq!(title_or_status(&curly), ("Don't".to_string(), TITLE));
    }
}
