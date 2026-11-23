//! The screens, drawn in the panel's coordinates.
//!
//! The picture uses 255 x 63 of the 256 x 64: the last column and row are
//! spare, so the pixel orbit can move everything by one without cutting
//! anything. Grey levels carry the hierarchy: the title brightest, then the
//! artist, then the album, with the chrome quieter than any of them.

use crate::figures;
use crate::frame::{self, Frame};
use crate::grey;
use crate::icons;
use crate::scroll::Marquee;
use crate::segments;
use crate::text::{self, Face};
use audiophonics_base::clock::LocalTime;
use audiophonics_meters::{self as meters, Bar};
use audiophonics_player::State;
use audiophonics_unifont::Unifont;
use embedded_graphics::mono_font::iso_8859_1::{FONT_6X10, FONT_7X13, FONT_9X18_BOLD};
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
pub const HAIRLINE_Y: u32 = 10;
const TITLE_Y: i32 = 12;
/// The rows the title has: its own face fills them, the wide one sits
/// centred in them.
const TITLE_ROWS: u32 = 18;
const ARTIST_Y: i32 = 31;
const ALBUM_Y: i32 = 45;
/// The foot of the screen: the rows from the album's line down to the
/// picture's last. The album, the time and meters lying under the track
/// share them, as [`foot`] works out.
const FOOT_TOP: u32 = ALBUM_Y as u32;
const FOOT_END: u32 = 63;
/// The rows the album's line takes at the top of the foot.
const ALBUM_ROWS: u32 = 10;
/// The row the time has along the bottom is as tall as its figures, and
/// never less than this.
const TIME_BAND: u32 = 8;
/// The bar that shows how far the track has played.
const BAR_ROWS: u32 = 2;
/// The clear columns between a time and the bars beside it.
const TIME_GAP: u32 = 6;
/// Lying meters are a channel, a row of marks and a channel. They are not
/// drawn in fewer rows than three a channel, and where they have room they
/// have a clear row either side of the marks.
const METERS_LEAST: u32 = 7;
const METERS_ROOMY: u32 = 15;
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
    /// Lying under the track, left channel over right, from edge to edge:
    /// the time in a row of its own under them.
    Under,
    /// Lying under the track, between the time played and the track's
    /// length.
    Between,
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

/// The time as the foot has it: the size of its figures, the row their
/// tops stand on, and the first row of the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Timed {
    size: figures::Size,
    top: u32,
    bar: u32,
}

/// Meters lying in the foot: the left channel's first row, the rows of a
/// channel, the clear rows either side of the marks, and whether they
/// stand between the two times or run from edge to edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Lain {
    top: u32,
    rows: u32,
    clear: u32,
    between: bool,
}

impl Lain {
    /// Meters in the rows from `top` to before `end`, standing on the last
    /// of them.
    fn within(top: u32, end: u32, between: bool) -> Self {
        let room = end - top;
        let clear = u32::from(room >= METERS_ROOMY);
        let rows = (room - 1 - 2 * clear) / 2;
        Self {
            top: end - Self::rows_of(rows, clear),
            rows,
            clear,
            between,
        }
    }

    fn rows_of(rows: u32, clear: u32) -> u32 {
        2 * rows + 1 + 2 * clear
    }

    fn marks(self) -> u32 {
        self.top + self.rows + self.clear
    }

    fn right(self) -> u32 {
        self.marks() + 1 + self.clear
    }

    /// The row after their last.
    #[cfg(test)]
    fn end(self) -> u32 {
        self.top + Self::rows_of(self.rows, self.clear)
    }
}

/// What stands in the foot, and where.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Foot {
    /// The album has a line of its own at the top of the foot. Without it
    /// the artist and the album share the line above.
    album: bool,
    time: Option<Timed>,
    meters: Option<Lain>,
}

/// The foot for the time and the meters that are asked for. Upright meters
/// stand beside the foot, not in it, and ask nothing of it.
///
/// The rules, in the order they are tried:
///
/// - The time has a row of its own along the bottom, as tall as its
///   figures, with the bar between the time played and the track's length.
/// - The album keeps its line where that row leaves it room. Where it does
///   not, the time has the whole foot: its figures stand in the middle of
///   it, and the bar on the middle of them.
/// - Meters lying under the track take the album's rows. With the time
///   under them they have what the time's row leaves, less a clear row; if
///   that is too little to draw them in, or the time is asked for beside
///   them, the time stands at either side in the middle of the foot, and
///   the meters and the bar between, where they are under the smallest
///   time.
/// - Without the time, lying meters have the whole foot.
fn foot(time: Option<figures::Size>, meters: Option<Place>) -> Foot {
    let lying = meters.filter(|p| matches!(p, Place::Under | Place::Between));
    let Some(size) = time else {
        return Foot {
            album: lying.is_none(),
            time: None,
            meters: lying.map(|_| Lain::within(FOOT_TOP, FOOT_END, false)),
        };
    };
    let rows = size.rows();
    let band = rows.max(TIME_BAND);
    let band_top = FOOT_END - band;
    let along = Timed {
        size,
        top: band_top + (band - rows) / 2,
        bar: band_top + (band - BAR_ROWS) / 2,
    };
    let middle = FOOT_TOP + (FOOT_END - FOOT_TOP - rows) / 2;
    match lying {
        None if band_top >= FOOT_TOP + ALBUM_ROWS => Foot {
            album: true,
            time: Some(along),
            meters: None,
        },
        None => Foot {
            album: false,
            time: Some(Timed {
                size,
                top: middle,
                bar: middle + (rows - BAR_ROWS) / 2,
            }),
            meters: None,
        },
        Some(Place::Under) if band_top > FOOT_TOP + METERS_LEAST => Foot {
            album: false,
            time: Some(along),
            meters: Some(Lain::within(FOOT_TOP, band_top - 1, false)),
        },
        Some(_) => {
            let least_top = FOOT_END - TIME_BAND;
            Foot {
                album: false,
                time: Some(Timed {
                    size,
                    top: middle,
                    bar: least_top + (TIME_BAND - BAR_ROWS) / 2,
                }),
                meters: Some(Lain::within(FOOT_TOP, least_top - 1, true)),
            }
        }
    }
}

/// The grey of the time: its smallest figures as quiet as the album, as
/// they have always been, and the larger ones as the artist, under the
/// title still.
fn time_grey(size: figures::Size) -> u8 {
    match size {
        figures::Size::Small => ALBUM,
        _ => ARTIST,
    }
}

/// The columns something runs over, from the first to before the last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run {
    from: u32,
    to: u32,
}

impl Run {
    fn is_empty(self) -> bool {
        self.to <= self.from
    }

    /// The column of a place along it: the floor of a meter at its first
    /// column, full scale at its last.
    fn column(self, place: f32) -> u32 {
        let last = self.to.saturating_sub(self.from + 1);
        self.from + (place.clamp(0.0, 1.0) * last as f32).round() as u32
    }
}

/// The time played at the left of `width` columns, the track's length at
/// the right, and the bar between them. Both are as wide as the length, so
/// that nothing between them moves while a track plays. Returns the columns
/// between the two, or `None` for what has no length: a stream, or no
/// track.
fn time(f: &mut Frame, s: &State, width: u32, t: Timed, unlit: u8) -> Option<Run> {
    let (Some(seek), Some(duration)) = (s.seek, s.duration) else {
        return None;
    };
    if duration == 0 {
        return None;
    }
    let played = (seek / 1000).min(duration);
    let length = mmss(duration);
    let elapsed = format!("{:>1$}", mmss(played), length.chars().count());
    let w = t.size.width(&length);
    let right = width.saturating_sub(1 + w);
    let luma = time_grey(t.size);
    t.size.draw(f, 1, t.top, &elapsed, luma, unlit);
    t.size.draw(f, right, t.top, &length, luma, unlit);

    let run = Run {
        from: 1 + w + 1 + TIME_GAP,
        to: right.saturating_sub(TIME_GAP),
    };
    if !run.is_empty() {
        let fill = run.from + (played * u64::from(run.to - run.from) / duration) as u32;
        for px in run.from..run.to {
            let l = if px < fill { BAR_FILL } else { BAR_TRACK };
            for row in 0..BAR_ROWS {
                f.set(px, t.bar + row, l);
            }
        }
    }
    Some(run)
}

/// One channel: bright as far as its level, dimmer as far as its peak, the
/// faintest grey for the rest, and the hold as a mark two columns wide.
fn meter(f: &mut Frame, run: Run, y: u32, rows: u32, bar: &Bar) {
    let level = run.column(bar.level);
    let peak = run.column(bar.peak).max(level);
    for x in run.from..run.to {
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
        let hold = run.column(bar.hold).max(run.from + 1);
        for x in hold - 1..=hold {
            for row in 0..rows {
                f.set(x, y + row, METER_HOLD);
            }
        }
    }
}

/// The two channels over `run`, left over right, with the scale's marks
/// between them, in the rows the foot gives them.
fn meter_pair(f: &mut Frame, bars: &[Bar; 2], lain: Lain, run: Run) {
    if run.to < run.from + 2 {
        return;
    }
    meter(f, run, lain.top, lain.rows, &bars[0]);
    meter(f, run, lain.right(), lain.rows, &bars[1]);
    for db in METER_MARKS_DB {
        f.set(run.column(meters::place(db)), lain.marks(), METER_MARK);
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
#[derive(Debug)]
pub struct Playing {
    title: Marquee,
    second: Marquee,
    /// The time played, the track's length and the bar between them: how
    /// large, or left out.
    time: Option<figures::Size>,
    /// The grey of the segments that are not lit, where the time's figures
    /// have segments.
    unlit: u8,
}

impl Default for Playing {
    fn default() -> Self {
        Self {
            title: Marquee::default(),
            second: Marquee::default(),
            time: Some(figures::Size::Small),
            unlit: 0,
        }
    }
}

impl Playing {
    /// How large the time is drawn, or `None` to leave it out. It is drawn
    /// at its smallest unless this says otherwise.
    pub fn show_time(&mut self, size: Option<figures::Size>) {
        self.time = size;
    }

    /// The grey of the unlit segments of a time drawn in segments: the
    /// clock's.
    pub fn unlit(&mut self, luma: u8) {
        self.unlit = luma;
    }

    /// The track, and the level meters where they are asked for: lying
    /// under the track, where the artist and the album share a line to make
    /// room, or upright at one side, where every line is that much shorter.
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
            None => self.lines(f, s, now, font, W, None),
            Some(Metered {
                place: Place::Under | Place::Between,
                ..
            }) => self.lines(f, s, now, font, W, meters),
            Some(Metered { bars, place }) => {
                // The lines are drawn narrower and set down beside the
                // meters.
                let width = W - STRIP;
                let mut body = Frame::new();
                self.lines(&mut body, s, now, font, width, None);
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

    /// The title, the artist and the album, the time, and meters lying
    /// under the track, within `width` columns from the left edge: the
    /// title and the line under it where they always are, and the rest as
    /// the foot has it.
    fn lines(
        &mut self,
        f: &mut Frame,
        s: &State,
        now: Instant,
        font: &mut Unifont,
        width: u32,
        lying: Option<Metered>,
    ) {
        let foot = foot(self.time, lying.map(|m| m.place));
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
        if foot.album && text::latin(&artist) && text::latin(&album) {
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
        let between = foot
            .time
            .and_then(|timed| time(f, s, width, timed, self.unlit));
        if let (Some(lain), Some(Metered { bars, .. })) = (foot.meters, lying) {
            let run = match between {
                Some(run) if lain.between => run,
                _ => Run { from: 0, to: width },
            };
            meter_pair(f, bars, lain, run);
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
    use crate::figures::{Face, Size, FACES, SIZES};
    use crate::frame;
    use audiophonics_unifont::sample;

    /// Where the smallest time stands, which is where the time has always
    /// stood: the first row of its own row, and the bar's.
    const TIME_Y: u32 = 55;
    const BAR_Y: u32 = 58;
    /// The picture from edge to edge.
    const WHOLE: Run = Run { from: 0, to: W };

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
            (1..21).any(|x| f.get(x, TIME_Y + 2) > 0),
            "elapsed at the left"
        );
        assert!(
            (235..W).any(|x| f.get(x, TIME_Y + 2) > 0),
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
            (0..W).all(|x| (48..TIME_Y).all(|y| f.get(x, y) == 0)),
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
        let lain = Lain::within(FOOT_TOP, TIME_Y - 1, false);
        assert_eq!((lain.top, lain.rows, lain.clear), (45, 4, 0));
        meter_pair(&mut f, &bars(0.25, 0.5, 0.75), lain, WHOLE);
        let y = lain.top;
        let (level, peak, hold) = (WHOLE.column(0.25), WHOLE.column(0.5), WHOLE.column(0.75));
        for row in 0..lain.rows {
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
        assert!((0..W).all(|x| f.get(x, lain.right()) == METER_REST));
        // The marks stand between the two, the last at full scale.
        assert_eq!(f.get(W - 1, lain.marks()), METER_MARK);
        assert_eq!(
            f.get(WHOLE.column(meters::place(-20.0)), lain.marks()),
            METER_MARK
        );
        assert_eq!(f.get(0, lain.marks()), 0);
    }

    #[test]
    fn a_meter_between_two_columns_keeps_within_them() {
        let run = Run { from: 40, to: 200 };
        let lain = Lain::within(FOOT_TOP, TIME_Y - 1, true);
        let mut f = Frame::new();
        meter_pair(&mut f, &bars(1.0, 1.0, 1.0), lain, run);
        for y in lain.top..lain.end() {
            assert!((0..40).chain(200..frame::WIDTH).all(|x| f.get(x, y) == 0));
        }
        // The floor at its first column, full scale and the hold at its last.
        assert_eq!(f.get(40, lain.top), METER_LEVEL);
        assert_eq!(f.get(199, lain.top), METER_HOLD);
        assert_eq!(f.get(199, lain.marks()), METER_MARK);
        assert_eq!(run.column(0.0), 40);
        assert_eq!(run.column(1.0), 199);
        // A hold at the floor is still two columns, and within the run.
        let mut low = Frame::new();
        meter_pair(&mut low, &bars(0.0, 0.0, 0.001), lain, run);
        assert_eq!(
            (low.get(40, lain.top), low.get(41, lain.top)),
            (METER_HOLD, METER_HOLD)
        );
        assert_eq!(low.get(39, lain.top), 0);
        // No room at all: nothing drawn, and nothing goes wrong.
        let mut none = Frame::new();
        meter_pair(
            &mut none,
            &bars(0.5, 0.5, 0.5),
            lain,
            Run { from: 90, to: 90 },
        );
        assert_eq!(none, Frame::new());
    }

    #[test]
    fn a_peak_at_full_scale_holds_at_the_right_edge_and_stays_in_the_picture() {
        let mut f = Frame::new();
        let lain = Lain::within(FOOT_TOP, TIME_Y - 1, false);
        meter_pair(&mut f, &bars(1.0, 1.0, 1.0), lain, WHOLE);
        assert_eq!(f.get(W - 1, lain.top), METER_HOLD);
        assert_eq!(f.get(W - 3, lain.top), METER_LEVEL);
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
        for y in (0..ARTIST_Y as u32).chain(TIME_Y..frame::HEIGHT) {
            for x in 0..frame::WIDTH {
                assert_eq!(plain.get(x, y), metered.get(x, y), "({x}, {y})");
            }
        }
        // The second line carries more than the artist: the album is on it.
        let lit = |f: &Frame| {
            (0..W)
                .filter(|&x| (ARTIST_Y as u32..FOOT_TOP).any(|y| f.get(x, y) != 0))
                .count()
        };
        assert!(lit(&metered) > lit(&plain));
        assert_eq!(metered.get(0, FOOT_TOP), METER_LEVEL);
        // The meters end a row above the time row.
        assert!((0..W).all(|x| metered.get(x, TIME_Y - 1) == 0));
        assert_eq!(metered.get(W - 1, TIME_Y - 2), METER_REST);
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
        assert!((0..100).any(|x| (ALBUM_Y as u32..TIME_Y).any(|y| right.get(x, y) == ALBUM)));
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
        screen.show_time(None);
        screen.render(&mut without, &s, "10.0.0.1", now, &mut none, None);
        // The same picture above the time row, and nothing where it was.
        for y in 0..frame::HEIGHT {
            for x in 0..frame::WIDTH {
                if y < TIME_Y {
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
        let lain = Lain::within(FOOT_TOP, FOOT_END, false);
        assert_eq!((lain.top, lain.rows, lain.clear), (46, 7, 1));
        for row in 0..lain.rows {
            assert_eq!(tall.get(0, lain.top + row), METER_LEVEL);
            assert_eq!(tall.get(W - 1, lain.right() + row), METER_REST);
        }
        // A clear row between the line above and the meters, and either
        // side of the marks; the bottom row stays dark for the orbit.
        assert!((0..W).all(|x| tall.get(x, lain.top - 1) == 0));
        assert!((0..W).all(|x| tall.get(x, lain.marks() - 1) == 0));
        assert!((0..W).all(|x| tall.get(x, lain.marks() + 1) == 0));
        assert_eq!(tall.get(W - 1, lain.marks()), METER_MARK);
        assert_eq!(lain.end(), 63);
        // Beside or under is the time's to say: without it, the same.
        let between = Metered {
            bars: &b,
            place: Place::Between,
        };
        let mut same = Frame::new();
        screen.render(&mut same, &s, "10.0.0.1", now, &mut none, Some(between));
        assert_eq!(same, tall);
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
        assert!((STRIP..W).all(|x| (TIME_Y..frame::HEIGHT).all(|y| upright.get(x, y) == 0)));
        assert_eq!(upright.get(0, COLUMN_BOTTOM), METER_LEVEL);
    }

    const PLACES: [Option<Place>; 5] = [
        None,
        Some(Place::Under),
        Some(Place::Between),
        Some(Place::Left),
        Some(Place::Right),
    ];

    fn timed(size: Size, top: u32, bar: u32) -> Option<Timed> {
        Some(Timed { size, top, bar })
    }

    fn lain(top: u32, rows: u32, clear: u32, between: bool) -> Option<Lain> {
        Some(Lain {
            top,
            rows,
            clear,
            between,
        })
    }

    #[test]
    fn the_foot_is_worked_out_by_rule_for_every_time_and_every_place() {
        use Size::{Large, Medium, Small};
        #[allow(non_snake_case)]
        let ExtraLarge = Size::ExtraLarge(Face::Segments);
        let under = Some(Place::Under);
        let between = Some(Place::Between);
        // The time, the meters; the album's own line, the time's top and
        // bar, the meters.
        let table = [
            // No time: the album's line, or lying meters in the whole foot.
            (None, None, true, None, None),
            (None, under, false, None, lain(46, 7, 1, false)),
            (None, between, false, None, lain(46, 7, 1, false)),
            // No lying meters: the two smaller sizes under the album's
            // line, the two larger in the middle of the foot.
            (Some(Small), None, true, timed(Small, 56, 58), None),
            (Some(Medium), None, true, timed(Medium, 55, 58), None),
            (Some(Large), None, false, timed(Large, 49, 53), None),
            (
                Some(ExtraLarge),
                None,
                false,
                timed(ExtraLarge, 47, 52),
                None,
            ),
            // The time under lying meters: they have what its row leaves.
            (
                Some(Small),
                under,
                false,
                timed(Small, 56, 58),
                lain(45, 4, 0, false),
            ),
            (
                Some(Medium),
                under,
                false,
                timed(Medium, 55, 58),
                lain(45, 4, 0, false),
            ),
            (
                Some(Large),
                under,
                false,
                timed(Large, 53, 57),
                lain(45, 3, 0, false),
            ),
            // The largest leaves them too little, and stands beside them.
            (
                Some(ExtraLarge),
                under,
                false,
                timed(ExtraLarge, 47, 58),
                lain(45, 4, 0, true),
            ),
            // The time beside lying meters: in the middle of the foot.
            (
                Some(Small),
                between,
                false,
                timed(Small, 51, 58),
                lain(45, 4, 0, true),
            ),
            (
                Some(Medium),
                between,
                false,
                timed(Medium, 50, 58),
                lain(45, 4, 0, true),
            ),
            (
                Some(Large),
                between,
                false,
                timed(Large, 49, 58),
                lain(45, 4, 0, true),
            ),
            (
                Some(ExtraLarge),
                between,
                false,
                timed(ExtraLarge, 47, 58),
                lain(45, 4, 0, true),
            ),
        ];
        assert_eq!(table.len(), 15, "five times by three ways to lie or not");
        for (time, place, album, timed, lain) in table {
            assert_eq!(
                foot(time, place),
                Foot {
                    album,
                    time: timed,
                    meters: lain
                },
                "{time:?} {place:?}"
            );
        }
        // The largest has the same foot whatever its face.
        for face in FACES {
            for place in [None, under, between] {
                let mut segments = foot(Some(ExtraLarge), place);
                let other = foot(Some(Size::ExtraLarge(face)), place);
                segments.time.as_mut().unwrap().size = Size::ExtraLarge(face);
                assert_eq!(other, segments, "{face:?} {place:?}");
            }
        }
        // Upright meters stand beside the foot and ask nothing of it.
        for time in SIZES.map(Some).into_iter().chain([None]) {
            for upright in [Some(Place::Left), Some(Place::Right)] {
                assert_eq!(foot(time, upright), foot(time, None), "{time:?}");
            }
        }
    }

    #[test]
    fn whatever_is_asked_for_nothing_in_the_foot_shares_a_row_it_should_not() {
        for time in SIZES.map(Some).into_iter().chain([None]) {
            for place in PLACES {
                let foot = foot(time, place);
                let what = format!("{time:?} {place:?}");
                if let Some(t) = foot.time {
                    let rows = t.size.rows();
                    assert!(t.top >= FOOT_TOP && t.top + rows <= FOOT_END, "{what}");
                    assert!(t.bar >= FOOT_TOP && t.bar + BAR_ROWS <= FOOT_END, "{what}");
                    if foot.album {
                        // A clear row is the album's own: its face keeps
                        // the last row of its line for what hangs down.
                        assert!(t.top >= FOOT_TOP + ALBUM_ROWS, "{what}");
                    }
                }
                if let Some(m) = foot.meters {
                    assert!(!foot.album, "{what}");
                    assert!(m.rows >= 3, "{what}");
                    assert!(m.top >= FOOT_TOP && m.end() <= FOOT_END, "{what}");
                    assert_eq!(m.right() + m.rows, m.end(), "{what}");
                    match foot.time {
                        // Edge to edge: a clear row, then the time's row.
                        Some(t) if !m.between => {
                            assert!(m.end() < t.top && m.end() < t.bar, "{what}");
                        }
                        // Between the times: a clear row, then the bar.
                        Some(t) => assert!(m.end() < t.bar, "{what}"),
                        None => assert_eq!(m.end(), FOOT_END, "{what}"),
                    }
                }
                // Without meters in its way the bar is on the rows of the
                // figures' middle.
                if let (Some(t), None) = (foot.time, foot.meters) {
                    let middle = 2 * t.top + t.size.rows();
                    assert!((2 * t.bar + BAR_ROWS).abs_diff(middle) <= 1, "{what}");
                }
            }
        }
    }

    /// A track twenty minutes long, a little way in, or at `seek` seconds.
    fn long_track(seek: u64) -> State {
        State {
            seek: Some(seek * 1000),
            duration: Some(1200),
            ..metered_state()
        }
    }

    fn drawn(time: Option<Size>, place: Option<Place>, s: &State) -> Frame {
        let mut none = Unifont::new("/nonexistent/unifont.hex");
        let b = bars(0.5, 0.6, 0.7);
        let mut screen = Playing::default();
        screen.show_time(time);
        let mut f = Frame::new();
        let meters = place.map(|place| Metered { bars: &b, place });
        screen.render(&mut f, s, "10.0.0.1", Instant::now(), &mut none, meters);
        f
    }

    /// The columns the lines have, and where they begin, beside upright
    /// meters or without them.
    fn lines_at(place: Option<Place>) -> (u32, u32) {
        match place {
            Some(Place::Left) => (STRIP, W - STRIP),
            Some(Place::Right) => (0, W - STRIP),
            _ => (0, W),
        }
    }

    #[test]
    fn every_time_in_every_place_is_drawn_whole_and_clear_of_the_rest() {
        let s = metered_state();
        for size in SIZES {
            for place in PLACES {
                let what = format!("{size:?} {place:?}");
                let f = drawn(Some(size), place, &s);
                let foot = foot(Some(size), place);
                let t = foot.time.unwrap();
                let (x, width) = lines_at(place);
                let w = size.width("5:12");
                let luma = time_grey(size);
                let rows = t.top..t.top + size.rows();

                // The spare column and row stay dark for the orbit.
                assert!((0..frame::HEIGHT).all(|y| f.get(255, y) == 0), "{what}");
                assert!((0..frame::WIDTH).all(|x| f.get(x, 63) == 0), "{what}");

                // Both times, in their grey, from their first row to their
                // last: the time played from the lines' first column but
                // one, the length to their last but one.
                let (played, length) = (x + 1..x + 1 + w, x + width - 1 - w..x + width - 1);
                for columns in [played.clone(), length.clone()] {
                    for y in [rows.start, rows.end - 1] {
                        assert!(
                            columns.clone().any(|x| f.get(x, y) == luma),
                            "{what} row {y}"
                        );
                    }
                }
                // Nothing of the screen touches a figure: the row above
                // them and the row below are clear, and so are the columns
                // between them and the bars.
                for columns in [
                    x..played.end + 1 + TIME_GAP,
                    length.start - TIME_GAP..x + width,
                ] {
                    for y in [rows.start - 1, rows.end] {
                        assert!(columns.clone().all(|x| f.get(x, y) == 0), "{what} row {y}");
                    }
                }
                for columns in [
                    played.end..played.end + 1 + TIME_GAP,
                    length.start - TIME_GAP..length.start,
                ] {
                    assert!(
                        rows.clone()
                            .all(|y| columns.clone().all(|x| f.get(x, y) == 0)),
                        "{what} beside the figures"
                    );
                }

                // The bar between them, as long as the room there is, and
                // as far along as the track.
                let bar: Vec<u32> = (0..W)
                    .filter(|&x| [BAR_FILL, BAR_TRACK].contains(&f.get(x, t.bar)))
                    .collect();
                let run = Run {
                    from: played.end + 1 + TIME_GAP,
                    to: length.start - TIME_GAP,
                };
                if foot.meters.is_none() {
                    assert_eq!(bar, (run.from..run.to).collect::<Vec<_>>(), "{what}");
                }
                let filled = (run.from..run.to)
                    .filter(|&x| f.get(x, t.bar) == BAR_FILL)
                    .count();
                assert_eq!(
                    filled as u64,
                    63 * u64::from(run.to - run.from) / 312,
                    "{what}"
                );
                assert!((run.from..run.to).all(|x| f.get(x, t.bar + 1) == f.get(x, t.bar)));

                // Lying meters: from edge to edge over the time, or between
                // the two times and no further.
                if let Some(m) = foot.meters {
                    let across = if m.between { run } else { WHOLE };
                    for y in (m.top..m.top + m.rows).chain(m.right()..m.end()) {
                        assert_ne!(f.get(across.from, y), 0, "{what} row {y}");
                        assert_ne!(f.get(across.to - 1, y), 0, "{what} row {y}");
                    }
                    assert_eq!(f.get(across.from, m.top), METER_LEVEL, "{what}");
                    assert_eq!(f.get(across.to - 1, m.marks()), METER_MARK, "{what}");
                    if !m.between {
                        // A clear row between them and the time under them.
                        assert!((0..W).all(|x| f.get(x, m.end()) == 0), "{what}");
                    }
                }
                // The album has its line only where the foot leaves it one.
                let album = (x..x + 100)
                    .any(|x| (FOOT_TOP..FOOT_TOP + ALBUM_ROWS).any(|y| f.get(x, y) == ALBUM));
                if foot.album {
                    assert!(album, "{what}");
                }
            }
        }
    }

    #[test]
    fn both_times_are_as_wide_as_the_length_so_nothing_between_them_moves() {
        let bar_and_meters = |f: &Frame, foot: Foot| -> Vec<(u32, u32)> {
            let t = foot.time.unwrap();
            let rows =
                (t.bar..t.bar + BAR_ROWS).chain(foot.meters.map_or(0..0, |m| m.top..m.end()));
            rows.flat_map(|y| {
                (0..W)
                    .filter(move |&x| f.get(x, y) != 0)
                    .map(move |x| (x, y))
            })
            .filter(|&(x, y)| ![time_grey(t.size)].contains(&f.get(x, y)))
            .collect()
        };
        for size in SIZES {
            for place in [
                None,
                Some(Place::Under),
                Some(Place::Between),
                Some(Place::Left),
            ] {
                let what = format!("{size:?} {place:?}");
                let foot = foot(Some(size), place);
                // A second short of ten minutes, and at ten: the time
                // played has grown a figure.
                let before = drawn(Some(size), place, &long_track(599));
                let after = drawn(Some(size), place, &long_track(600));
                let (b, a) = (bar_and_meters(&before, foot), bar_and_meters(&after, foot));
                let columns = |lit: &[(u32, u32)]| {
                    (lit.iter().map(|p| p.0).min(), lit.iter().map(|p| p.0).max())
                };
                assert_eq!(columns(&b), columns(&a), "{what}");

                // The shorter time ends where the longer one does, and
                // leaves its first figure's columns dark.
                let (x, _) = lines_at(place);
                let t = foot.time.unwrap();
                let rows = t.top..t.top + size.rows();
                let lead = size.width("10:00") - size.width("9:59");
                assert!(lead >= 5, "{what}");
                assert!(
                    (x..x + lead).all(|x| rows.clone().all(|y| before.get(x, y) == 0)),
                    "{what}"
                );
                let last = |f: &Frame| {
                    (x..x + 1 + size.width("10:00") + 1)
                        .filter(|&x| rows.clone().any(|y| f.get(x, y) != 0))
                        .max()
                };
                assert_eq!(last(&before), last(&after), "{what}");
            }
        }
    }

    #[test]
    fn the_largest_time_asked_for_under_lying_meters_stands_beside_them() {
        let s = metered_state();
        for face in FACES {
            assert_eq!(
                drawn(Some(Size::ExtraLarge(face)), Some(Place::Under), &s),
                drawn(Some(Size::ExtraLarge(face)), Some(Place::Between), &s),
                "{face:?}"
            );
        }
        // The sizes that fit under them are not the same beside them.
        for size in [Size::Small, Size::Medium, Size::Large] {
            assert_ne!(
                drawn(Some(size), Some(Place::Under), &s),
                drawn(Some(size), Some(Place::Between), &s),
                "{size:?}"
            );
        }
    }

    #[test]
    fn what_has_no_length_has_no_time_and_lying_meters_then_run_from_edge_to_edge() {
        let mut radio = metered_state();
        radio.duration = Some(0);
        for size in SIZES {
            for place in [Place::Under, Place::Between] {
                let f = drawn(Some(size), Some(place), &radio);
                let m = foot(Some(size), Some(place)).meters.unwrap();
                assert_eq!(f.get(0, m.top), METER_LEVEL, "{size:?} {place:?}");
                assert_eq!(f.get(W - 1, m.right()), METER_REST, "{size:?} {place:?}");
                // No figures and no bar: under the meters, nothing.
                assert!(
                    (m.end()..frame::HEIGHT).all(|y| (0..W).all(|x| f.get(x, y) == 0)),
                    "{size:?} {place:?}"
                );
            }
        }
    }

    #[test]
    fn the_larger_times_are_drawn_in_the_artists_grey_and_the_largest_in_segments() {
        assert_eq!(time_grey(Size::Small), ALBUM);
        for size in SIZES.into_iter().filter(|&size| size != Size::Small) {
            assert_eq!(time_grey(size), ARTIST, "{size:?}");
        }
        // The segments that are not lit follow the clock's, in the largest
        // size only.
        let s = metered_state();
        let mut none = Unifont::new("/nonexistent/unifont.hex");
        let render = |size: Size, unlit: u8, none: &mut Unifont| {
            let mut screen = Playing::default();
            screen.show_time(Some(size));
            screen.unlit(unlit);
            let mut f = Frame::new();
            screen.render(&mut f, &s, "10.0.0.1", Instant::now(), none, None);
            f
        };
        let segments = Size::ExtraLarge(Face::Segments);
        let top = foot(Some(segments), None).time.unwrap().top;
        // "1:03": the one has no top segment, so its ghost shows there.
        assert_eq!(render(segments, 0, &mut none).get(3, top), 0);
        assert_eq!(
            render(segments, grey::FAINT, &mut none).get(3, top),
            grey::FAINT
        );
        for size in SIZES.into_iter().filter(|&size| size != segments) {
            assert_eq!(
                render(size, 0, &mut none),
                render(size, grey::FAINT, &mut none),
                "{size:?}"
            );
        }
    }
}
