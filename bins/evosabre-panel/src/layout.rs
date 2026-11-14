//! The screens, drawn in the panel's coordinates.
//!
//! The picture uses 255 x 63 of the 256 x 64: the last column and row are
//! spare, so the pixel orbit can move everything by one without cutting
//! anything. Grey levels carry the hierarchy: the title brightest, then the
//! artist, then the album, with the chrome quieter than any of them.

use crate::frame::{self, Frame};
use crate::grey;
use crate::icons;
use crate::meters::{self, Bar};
use crate::scroll::Marquee;
use crate::segments;
use crate::text::{self, Face};
use audiophonics_base::clock::LocalTime;
use audiophonics_player::State;
use audiophonics_unifont::Unifont;
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
/// A meter: its level, the reach of its peak beyond that, and the rest of
/// its length, which is there without drawing the eye.
pub const METER_LEVEL: u8 = grey::linear(10);
pub const METER_PEAK: u8 = grey::linear(4);
pub const METER_REST: u8 = grey::TRACE;
pub const METER_HOLD: u8 = grey::FULL;
pub const METER_MARK: u8 = grey::linear(5);

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
/// The meters take the album's rows: the left channel, a row of marks, the
/// right channel.
const METER_ROWS: u32 = 4;
const METER_LEFT_Y: u32 = 45;
const METER_MARKS_Y: u32 = METER_LEFT_Y + METER_ROWS;
const METER_RIGHT_Y: u32 = METER_MARKS_Y + 1;
/// Without the time row the meters have its rows too: taller, with a clear
/// row either side of the marks, down to the bottom row but one.
const METER_TALL_ROWS: u32 = 7;
const METER_TALL_LEFT_Y: u32 = 46;
const METER_TALL_MARKS_Y: u32 = METER_TALL_LEFT_Y + METER_TALL_ROWS + 1;
const METER_TALL_RIGHT_Y: u32 = METER_TALL_MARKS_Y + 2;
/// Where the marks stand, in decibels under full scale.
const METER_MARKS_DB: [f32; 8] = [-50.0, -40.0, -30.0, -20.0, -10.0, -6.0, -3.0, 0.0];
/// Upright, the meters stand from under the hairline to the bottom row but
/// one: the left channel, two columns of marks, the right channel.
const COLUMN_W: u32 = 5;
const COLUMN_MARKS_W: u32 = 2;
const COLUMN_TOP: u32 = HAIRLINE_Y + 2;
const COLUMN_BOTTOM: u32 = 62;
/// The width the upright meters take, and that with the gap between them
/// and the lines beside them.
const STRIP_BARS: u32 = 2 * COLUMN_W + COLUMN_MARKS_W;
const STRIP: u32 = STRIP_BARS + 3;

/// Where the meters stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Lying under the track, left channel over right.
    Under,
    /// Upright at the left of the track.
    Left,
    /// Upright at the right of the track.
    Right,
}

/// The meters to draw: their marks and where they stand.
#[derive(Debug, Clone, Copy)]
pub struct Metered<'a> {
    pub bars: &'a [Bar; 2],
    pub place: Place,
}

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

    // The speaker is crossed at volume 0 and not for the player's mute
    // flag, which with this DAC says muted while music plays.
    let silent = s.volume == Some(0);
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

/// Elapsed at the left, length at the right of `width` columns, the bar
/// between them.
fn time_row(f: &mut Frame, s: &State, width: u32) {
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
    mono(f, (width - lw) as i32, TIME_Y, &length, TIME_FONT, ALBUM);

    let x0 = 1 + ew + 6;
    let x1 = (width - lw).saturating_sub(6);
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

/// The column of a place along a meter: the floor at the left edge, full
/// scale at the right.
fn meter_x(place: f32) -> u32 {
    (place.clamp(0.0, 1.0) * (W - 1) as f32).round() as u32
}

/// One channel: bright as far as its level, dimmer as far as its peak, the
/// faintest grey for the rest, and the hold as a mark two columns wide.
fn meter(f: &mut Frame, y: u32, rows: u32, bar: &Bar) {
    let level = meter_x(bar.level);
    let peak = meter_x(bar.peak).max(level);
    for x in 0..W {
        let luma = if x < level {
            METER_LEVEL
        } else if x < peak {
            METER_PEAK
        } else {
            METER_REST
        };
        for row in 0..rows {
            f.set(x, y + row, luma);
        }
    }
    if bar.hold > 0.0 {
        let hold = meter_x(bar.hold).max(1);
        for x in hold - 1..=hold {
            for row in 0..rows {
                f.set(x, y + row, METER_HOLD);
            }
        }
    }
}

/// The two channels, left over right, with the scale's marks between them:
/// in the album's rows, or `tall`, in those and the time row's.
fn meter_pair(f: &mut Frame, bars: &[Bar; 2], tall: bool) {
    let (left, marks, right, rows) = if tall {
        (
            METER_TALL_LEFT_Y,
            METER_TALL_MARKS_Y,
            METER_TALL_RIGHT_Y,
            METER_TALL_ROWS,
        )
    } else {
        (METER_LEFT_Y, METER_MARKS_Y, METER_RIGHT_Y, METER_ROWS)
    };
    meter(f, left, rows, &bars[0]);
    meter(f, right, rows, &bars[1]);
    for db in METER_MARKS_DB {
        f.set(meter_x(meters::place(db)), marks, METER_MARK);
    }
}

/// How many rows above the bottom a place along an upright meter is.
fn meter_rows(place: f32) -> u32 {
    (place.clamp(0.0, 1.0) * (COLUMN_BOTTOM - COLUMN_TOP) as f32).round() as u32
}

/// One channel upright, the floor at the bottom: the same greys as lying
/// down, and the hold as a mark two rows high.
fn meter_column(f: &mut Frame, x: u32, bar: &Bar) {
    let level = meter_rows(bar.level);
    let peak = meter_rows(bar.peak).max(level);
    for up in 0..=COLUMN_BOTTOM - COLUMN_TOP {
        let luma = if up < level {
            METER_LEVEL
        } else if up < peak {
            METER_PEAK
        } else {
            METER_REST
        };
        for column in 0..COLUMN_W {
            f.set(x + column, COLUMN_BOTTOM - up, luma);
        }
    }
    if bar.hold > 0.0 {
        let hold = meter_rows(bar.hold).max(1);
        for up in hold - 1..=hold {
            for column in 0..COLUMN_W {
                f.set(x + column, COLUMN_BOTTOM - up, METER_HOLD);
            }
        }
    }
}

/// The two channels upright from column `x`, left beside right, with the
/// scale's marks between them.
fn meter_columns(f: &mut Frame, x: u32, bars: &[Bar; 2]) {
    meter_column(f, x, &bars[0]);
    meter_column(f, x + COLUMN_W + COLUMN_MARKS_W, &bars[1]);
    for db in METER_MARKS_DB {
        let y = COLUMN_BOTTOM - meter_rows(meters::place(db));
        for column in 0..COLUMN_MARKS_W {
            f.set(x + COLUMN_W + column, y, METER_MARK);
        }
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
    /// The time row left out: the time played, the track's length and the
    /// bar between them.
    no_time: bool,
}

impl Playing {
    /// Whether the time row is drawn. It is unless this says otherwise.
    pub fn show_time(&mut self, shown: bool) {
        self.no_time = !shown;
    }

    /// The track, and the level meters where they are asked for: under the
    /// track, where the artist and the album share a line to make room, or
    /// upright at one side, where every line is that much shorter.
    pub fn render(
        &mut self,
        f: &mut Frame,
        s: &State,
        ip: &str,
        now: Instant,
        font: &mut Unifont,
        meters: Option<Metered>,
    ) {
        chrome(f, s, ip, true);
        match meters {
            None => self.lines(f, s, now, font, W, false),
            Some(Metered {
                bars,
                place: Place::Under,
            }) => {
                self.lines(f, s, now, font, W, true);
                meter_pair(f, bars, self.no_time);
            }
            Some(Metered { bars, place }) => {
                // The lines are drawn narrower and set down beside the
                // meters.
                let width = W - STRIP;
                let mut body = Frame::new();
                self.lines(&mut body, s, now, font, width, false);
                let (text_x, meters_x) = match place {
                    Place::Left => (STRIP, 0),
                    _ => (0, W - STRIP_BARS),
                };
                for y in HAIRLINE_Y + 1..frame::HEIGHT {
                    for x in 0..width {
                        let luma = body.get(x, y);
                        if luma != 0 {
                            f.set(text_x + x, y, luma);
                        }
                    }
                }
                meter_columns(f, meters_x, bars);
            }
        }
    }

    /// The title, the artist and the album, and the time row, within
    /// `width` columns from the left edge. With `together` the artist and
    /// the album share one line and the album's rows are left free.
    fn lines(
        &mut self,
        f: &mut Frame,
        s: &State,
        now: Instant,
        font: &mut Unifont,
        width: u32,
        together: bool,
    ) {
        let (title, luma) = title_or_status(s);
        let face = Face::choose(&title, &FONT_9X18_BOLD, font);
        self.title.set(&title, face.width(&title, font), width, now);
        let drop = (TITLE_ROWS - face.height()) as i32 / 2;
        face.draw(
            f,
            font,
            &title,
            TITLE_Y + drop,
            luma,
            self.title.offset(now),
            width,
        );

        let artist = text::tidy(&s.artist);
        let album = text::tidy(&s.album);
        if !together && text::latin(&artist) && text::latin(&album) {
            let face = Face::Mono(&FONT_7X13);
            self.second
                .set(&artist, face.width(&artist, font), width, now);
            face.draw(
                f,
                font,
                &artist,
                ARTIST_Y,
                ARTIST,
                self.second.offset(now),
                width,
            );
            let album_chars = (width / FONT_6X10.character_size.width) as usize;
            let album = cut(&album, album_chars);
            Face::Mono(&FONT_6X10).draw(f, font, &album, ALBUM_Y, ALBUM, 0, width);
        } else {
            let both = joined(&artist, &album);
            let face = Face::choose(&both, &FONT_7X13, font);
            self.second.set(&both, face.width(&both, font), width, now);
            face.draw(
                f,
                font,
                &both,
                ARTIST_Y,
                ARTIST,
                self.second.offset(now),
                width,
            );
        }
        if !self.no_time {
            time_row(f, s, width);
        }
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
    use audiophonics_unifont::sample;

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
        Playing::default().render(&mut f, s, ip, Instant::now(), &mut none, None);
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
    fn volume_zero_crosses_the_speaker_and_quietens_the_number() {
        let mut s = dire_straits();
        s.volume = Some(0);
        let f = render(&s, "192.168.1.42");
        let fr = &f;
        let brightest = (31..49)
            .flat_map(|x| (0..10).map(move |y| fr.get(x, y)))
            .max()
            .unwrap();
        assert_eq!(brightest, QUIET);
    }

    #[test]
    fn the_players_mute_flag_alone_changes_nothing() {
        let plain = render(&dire_straits(), "192.168.1.42");
        let mut s = dire_straits();
        s.mute = true;
        assert_eq!(render(&s, "192.168.1.42"), plain);
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
        p.render(&mut a, &s, "10.0.0.1", t0, &mut none, None);
        assert!(p.scrolling());
        let mut b = Frame::new();
        p.render(
            &mut b,
            &s,
            "10.0.0.1",
            t0 + Duration::from_millis(3000),
            &mut none,
            None,
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
        Playing::default().render(&mut f, &s, "10.0.0.1", Instant::now(), &mut font, None);
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
        Playing::default().render(&mut f, &s, "10.0.0.1", Instant::now(), &mut font, None);
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

    fn bars(level: f32, peak: f32, hold: f32) -> [Bar; 2] {
        [
            Bar { level, peak, hold },
            Bar {
                level: 0.0,
                peak: 0.0,
                hold: 0.0,
            },
        ]
    }

    #[test]
    fn a_meter_is_bright_to_its_level_dimmer_to_its_peak_and_marks_its_hold() {
        let mut f = Frame::new();
        meter_pair(&mut f, &bars(0.25, 0.5, 0.75), false);
        let y = METER_LEFT_Y;
        let (level, peak, hold) = (meter_x(0.25), meter_x(0.5), meter_x(0.75));
        for row in 0..METER_ROWS {
            assert_eq!(f.get(0, y + row), METER_LEVEL);
            assert_eq!(f.get(level - 1, y + row), METER_LEVEL);
            assert_eq!(f.get(level, y + row), METER_PEAK);
            assert_eq!(f.get(peak - 1, y + row), METER_PEAK);
            assert_eq!(f.get(peak, y + row), METER_REST);
            assert_eq!(f.get(hold - 1, y + row), METER_HOLD);
            assert_eq!(f.get(hold, y + row), METER_HOLD);
            assert_eq!(f.get(hold + 1, y + row), METER_REST);
            assert_eq!(f.get(W - 1, y + row), METER_REST);
        }
        // The silent channel is its faintest grey from end to end.
        assert!((0..W).all(|x| f.get(x, METER_RIGHT_Y) == METER_REST));
        // The marks stand between the two, the last at full scale.
        assert_eq!(f.get(W - 1, METER_MARKS_Y), METER_MARK);
        assert_eq!(
            f.get(meter_x(meters::place(-20.0)), METER_MARKS_Y),
            METER_MARK
        );
        assert_eq!(f.get(0, METER_MARKS_Y), 0);
    }

    #[test]
    fn a_peak_at_full_scale_holds_at_the_right_edge_and_stays_in_the_picture() {
        let mut f = Frame::new();
        meter_pair(&mut f, &bars(1.0, 1.0, 1.0), false);
        assert_eq!(f.get(W - 1, METER_LEFT_Y), METER_HOLD);
        assert_eq!(f.get(W - 3, METER_LEFT_Y), METER_LEVEL);
        assert!(
            (0..frame::HEIGHT).all(|y| f.get(255, y) == 0),
            "column 255 lit"
        );
    }

    #[test]
    fn with_meters_the_artist_and_album_share_a_line_and_the_time_row_stays() {
        let s = State {
            status: "play".into(),
            title: "So Far Away".into(),
            artist: "Dire Straits".into(),
            album: "Brothers in Arms".into(),
            seek: Some(63_000),
            duration: Some(312),
            volume: Some(45),
            ..State::default()
        };
        let mut none = Unifont::new("/nonexistent/unifont.hex");
        let now = Instant::now();
        let mut plain = Frame::new();
        Playing::default().render(&mut plain, &s, "10.0.0.1", now, &mut none, None);
        let mut metered = Frame::new();
        let b = bars(0.5, 0.6, 0.7);
        let under = Metered {
            bars: &b,
            place: Place::Under,
        };
        Playing::default().render(&mut metered, &s, "10.0.0.1", now, &mut none, Some(under));

        // Above the second line and from the time row down, the same picture.
        for y in (0..ARTIST_Y as u32).chain(TIME_Y as u32..frame::HEIGHT) {
            for x in 0..frame::WIDTH {
                assert_eq!(plain.get(x, y), metered.get(x, y), "({x}, {y})");
            }
        }
        // The second line carries more than the artist: the album is on it.
        let lit = |f: &Frame| {
            (0..W)
                .filter(|&x| (ARTIST_Y as u32..METER_LEFT_Y).any(|y| f.get(x, y) != 0))
                .count()
        };
        assert!(lit(&metered) > lit(&plain));
        assert_eq!(metered.get(0, METER_LEFT_Y), METER_LEVEL);
        // The meters end a row above the time row.
        assert!(METER_RIGHT_Y + METER_ROWS <= TIME_Y as u32);
    }

    fn metered_state() -> State {
        State {
            status: "play".into(),
            title: "So Far Away".into(),
            artist: "Dire Straits".into(),
            album: "Brothers in Arms".into(),
            seek: Some(63_000),
            duration: Some(312),
            volume: Some(45),
            ..State::default()
        }
    }

    #[test]
    fn an_upright_meter_rises_from_the_bottom_and_marks_its_hold() {
        let mut f = Frame::new();
        meter_columns(&mut f, 0, &bars(0.25, 0.5, 0.75));
        let (level, peak, hold) = (meter_rows(0.25), meter_rows(0.5), meter_rows(0.75));
        for x in 0..COLUMN_W {
            assert_eq!(f.get(x, COLUMN_BOTTOM), METER_LEVEL);
            assert_eq!(f.get(x, COLUMN_BOTTOM - (level - 1)), METER_LEVEL);
            assert_eq!(f.get(x, COLUMN_BOTTOM - level), METER_PEAK);
            assert_eq!(f.get(x, COLUMN_BOTTOM - peak), METER_REST);
            assert_eq!(f.get(x, COLUMN_BOTTOM - hold), METER_HOLD);
            assert_eq!(f.get(x, COLUMN_BOTTOM - hold + 1), METER_HOLD);
            assert_eq!(f.get(x, COLUMN_TOP), METER_REST);
        }
        // The silent channel beside it, and the marks between the two.
        let right = COLUMN_W + COLUMN_MARKS_W;
        assert!((COLUMN_TOP..=COLUMN_BOTTOM).all(|y| f.get(right, y) == METER_REST));
        assert_eq!(
            f.get(COLUMN_W, COLUMN_TOP),
            METER_MARK,
            "full scale at the top"
        );
        assert_eq!(f.get(COLUMN_W + 1, COLUMN_TOP), METER_MARK);
        // Nothing above the meters' top or under their bottom.
        assert!((0..STRIP_BARS).all(|x| f.get(x, COLUMN_TOP - 1) == 0 && f.get(x, 63) == 0));
    }

    #[test]
    fn upright_meters_stand_at_the_side_asked_for_and_the_lines_beside_them() {
        let s = metered_state();
        let mut none = Unifont::new("/nonexistent/unifont.hex");
        let now = Instant::now();
        let b = bars(0.5, 0.6, 0.7);

        let mut left = Frame::new();
        let at_left = Metered {
            bars: &b,
            place: Place::Left,
        };
        Playing::default().render(&mut left, &s, "10.0.0.1", now, &mut none, Some(at_left));
        assert_eq!(left.get(0, COLUMN_BOTTOM), METER_LEVEL);
        // A clear gap, then the title starts where the track's would at 0.
        assert!(
            (STRIP_BARS..STRIP).all(|x| (COLUMN_TOP..=COLUMN_BOTTOM).all(|y| left.get(x, y) == 0))
        );
        let mut plain = Frame::new();
        Playing::default().render(&mut plain, &s, "10.0.0.1", now, &mut none, None);
        for y in TITLE_Y as u32..ARTIST_Y as u32 {
            for x in 0..100 {
                assert_eq!(left.get(STRIP + x, y), plain.get(x, y), "({x}, {y})");
            }
        }

        let mut right = Frame::new();
        let at_right = Metered {
            bars: &b,
            place: Place::Right,
        };
        Playing::default().render(&mut right, &s, "10.0.0.1", now, &mut none, Some(at_right));
        assert_eq!(right.get(W - STRIP_BARS, COLUMN_BOTTOM), METER_LEVEL);
        assert_eq!(
            right.get(W - 1, COLUMN_BOTTOM),
            METER_REST,
            "the silent channel"
        );
        assert!(
            (0..frame::HEIGHT).all(|y| right.get(255, y) == 0),
            "column 255 lit"
        );
        // The lines are the track's own up to where the meters begin, and
        // stop short of them.
        for y in TITLE_Y as u32..ARTIST_Y as u32 {
            for x in 0..100 {
                assert_eq!(right.get(x, y), plain.get(x, y), "({x}, {y})");
            }
        }
        assert!((W - STRIP..W - STRIP_BARS)
            .all(|x| (COLUMN_TOP..=COLUMN_BOTTOM).all(|y| right.get(x, y) == 0)));
        // All three lines are there: the album has its own row again.
        assert!((0..100).any(|x| (ALBUM_Y as u32..TIME_Y as u32).any(|y| right.get(x, y) == ALBUM)));
        // The chrome is the track's from edge to edge.
        for y in 0..=HAIRLINE_Y {
            for x in 0..frame::WIDTH {
                assert_eq!(right.get(x, y), plain.get(x, y));
                assert_eq!(left.get(x, y), plain.get(x, y));
            }
        }
    }

    #[test]
    fn without_the_time_row_the_track_ends_at_the_album_and_the_meters_grow_into_its_rows() {
        let s = metered_state();
        let mut none = Unifont::new("/nonexistent/unifont.hex");
        let now = Instant::now();

        let mut with = Frame::new();
        Playing::default().render(&mut with, &s, "10.0.0.1", now, &mut none, None);
        let mut without = Frame::new();
        let mut screen = Playing::default();
        screen.show_time(false);
        screen.render(&mut without, &s, "10.0.0.1", now, &mut none, None);
        // The same picture above the time row, and nothing where it was.
        for y in 0..frame::HEIGHT {
            for x in 0..frame::WIDTH {
                if y < TIME_Y as u32 {
                    assert_eq!(with.get(x, y), without.get(x, y), "({x}, {y})");
                } else {
                    assert_eq!(without.get(x, y), 0, "({x}, {y})");
                }
            }
        }
        assert!(
            (0..W).any(|x| with.get(x, BAR_Y) != 0),
            "the time row is there by default"
        );

        let b = bars(0.5, 0.6, 0.7);
        let under = Metered {
            bars: &b,
            place: Place::Under,
        };
        let mut tall = Frame::new();
        screen.render(&mut tall, &s, "10.0.0.1", now, &mut none, Some(under));
        for row in 0..METER_TALL_ROWS {
            assert_eq!(tall.get(0, METER_TALL_LEFT_Y + row), METER_LEVEL);
            assert_eq!(tall.get(W - 1, METER_TALL_RIGHT_Y + row), METER_REST);
        }
        // A clear row between the line above and the meters, and either
        // side of the marks; the bottom row stays dark for the orbit.
        assert!((0..W).all(|x| tall.get(x, METER_TALL_LEFT_Y - 1) == 0));
        assert!((0..W).all(|x| tall.get(x, METER_TALL_MARKS_Y - 1) == 0));
        assert!((0..W).all(|x| tall.get(x, METER_TALL_MARKS_Y + 1) == 0));
        assert_eq!(tall.get(W - 1, METER_TALL_MARKS_Y), METER_MARK);
        assert_eq!(METER_TALL_RIGHT_Y + METER_TALL_ROWS, 63);
        assert!(
            (0..frame::WIDTH).all(|x| tall.get(x, 63) == 0),
            "row 63 lit"
        );

        // Upright meters lose the time row and nothing else.
        let at_left = Metered {
            bars: &b,
            place: Place::Left,
        };
        let mut upright = Frame::new();
        screen.render(&mut upright, &s, "10.0.0.1", now, &mut none, Some(at_left));
        assert!((STRIP..W).all(|x| (TIME_Y as u32..frame::HEIGHT).all(|y| upright.get(x, y) == 0)));
        assert_eq!(upright.get(0, COLUMN_BOTTOM), METER_LEVEL);
    }
}
