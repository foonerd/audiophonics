//! The panel's settings.
//!
//! The defaults are the EVO Sabre as it is wired: the OLED on SPI0 CE0 with
//! D/C on GPIO27 and reset on GPIO24. The plugin's settings page keeps the
//! user's choices in Volumio's configuration store, and this reads that
//! file; the wiring and the texts are read from the same file when a key
//! for them is there, for a board wired another way.
//!
//! A key that is absent leaves its default. A value that cannot be used is
//! named and leaves its default too: the panel comes up on a damaged store
//! and says in the journal what was wrong with it.

use crate::figures::Size;
use crate::grey;
use crate::layout::Place;
use audiophonics_base::log::Level;
use audiophonics_base::settings::Store;
use std::time::Duration;

pub const DEFAULT_PATH: &str =
    "/data/configuration/system_hardware/audiophonics_evo_sabre/config.json";
const PLUGIN_DIR: &str = "/data/plugins/system_hardware/audiophonics_evo_sabre";

/// Where the level meters stand when they are on: lying under the track,
/// with the time under them or beside them, or upright at one side of it.
pub fn meter_place(s: &str) -> Option<Place> {
    match s.trim() {
        "horizontal" => Some(Place::Under),
        "horizontal_beside" => Some(Place::Between),
        "vertical_left" => Some(Place::Left),
        "vertical_right" => Some(Place::Right),
        _ => None,
    }
}

/// How large the time of the track is drawn, as the settings name it:
/// `Some(None)` is their word for leaving it out.
pub fn time_size(s: &str) -> Option<Option<Size>> {
    match s.trim() {
        "off" => Some(None),
        other => Size::parse(other).map(Some),
    }
}

/// The segments of the clock's digits that are not lit: left dark, or
/// drawn in one of the panel's three faintest greys, as the ghost of an LED
/// digit. The largest time of the track is drawn in such digits too, and
/// follows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unlit {
    Off,
    VeryFaint,
    Faint,
    Dim,
}

impl Unlit {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "off" => Some(Self::Off),
            "very_faint" => Some(Self::VeryFaint),
            "faint" => Some(Self::Faint),
            "dim" => Some(Self::Dim),
            _ => None,
        }
    }

    /// The grey to draw them in: the two levels under the linear scale,
    /// or the linear scale's first visible step.
    pub fn level(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::VeryFaint => grey::TRACE,
            Self::Faint => grey::FAINT,
            Self::Dim => grey::linear(2),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub player_base: String,
    pub poll_interval: Duration,
    pub request_timeout: Duration,

    pub spi_device: String,
    pub spi_speed_hz: u32,
    pub dc_gpio: u32,
    pub reset_gpio: u32,
    /// Resolved by label, never by index: the numbering differs between the
    /// Pi 4 and Pi 5 families and has moved between kernels.
    pub gpio_chip_label: Option<String>,

    /// SSD1322 contrast current at full brightness, and at the dim level.
    pub contrast: u8,
    pub dim_contrast: u8,

    /// The greeting shown from start until the player answers, and for at
    /// least `greeting_hold`: the logo at `logo_path` (PGM, 256x64), or the
    /// text when there is no logo.
    pub greeting_text: String,
    pub greeting_hold: Duration,
    pub logo_path: String,
    /// What the panel says on a halt, and on a restart.
    pub farewell_text: String,
    pub restart_text: String,
    /// GNU Unifont as a `.hex` file, for text the panel's own faces lack.
    pub unifont_path: String,

    /// The panel dims after this long without a touch, and goes dark after
    /// this long while nothing plays. Zero disables a tier.
    pub dim_after: Duration,
    pub blank_after: Duration,
    /// How long the player must have rested, paused or stopped, before the
    /// clock replaces the track. Zero keeps the track for ever.
    pub clock_after: Duration,
    /// How the clock draws the segments of its digits that are not lit.
    pub clock_segments: Unlit,
    /// The level meters: off, or where they stand beside the track.
    pub meters: Option<Place>,
    /// Decibels added to what the meters show, -12 to 12.
    pub meters_gain: i8,
    /// The time played, the track's length and the bar between them: how
    /// large their figures are, or `None` to leave all three out.
    pub track_time: Option<Size>,
    /// Whether the snake has the screen while the player rests, in place
    /// of the clock.
    pub snake: bool,

    /// Whether the handset's transport keys are obeyed.
    pub remote: bool,
    /// How much the panel says in the journal: error, info or debug.
    pub log_level: Level,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            player_base: "http://127.0.0.1:3000".into(),
            poll_interval: Duration::from_millis(500),
            request_timeout: Duration::from_millis(400),

            spi_device: "/dev/spidev0.0".into(),
            spi_speed_hz: 4_800_000,
            dc_gpio: 27,
            reset_gpio: 24,
            gpio_chip_label: None,

            contrast: 100,
            dim_contrast: 24,

            greeting_text: "EVO SABRE".into(),
            greeting_hold: Duration::from_millis(2000),
            logo_path: format!("{PLUGIN_DIR}/assets/logo.pgm"),
            farewell_text: "Goodbye".into(),
            restart_text: "Back in a moment".into(),
            unifont_path: format!("{PLUGIN_DIR}/assets/unifont.hex"),

            dim_after: Duration::from_secs(1200),
            blank_after: Duration::from_secs(3600),
            clock_after: Duration::from_secs(6),
            clock_segments: Unlit::VeryFaint,
            meters: None,
            meters_gain: 0,
            track_time: Some(Size::Small),
            snake: false,

            remote: true,
            log_level: Level::Error,
        }
    }
}

impl Config {
    /// The contrast at the dim level: never brighter than the full one.
    pub fn dimmed(&self) -> u8 {
        self.dim_contrast.min(self.contrast)
    }

    /// True when `other` differs in something only read at start.
    pub fn rewired(&self, other: &Config) -> bool {
        self.spi_device != other.spi_device
            || self.spi_speed_hz != other.spi_speed_hz
            || self.dc_gpio != other.dc_gpio
            || self.reset_gpio != other.reset_gpio
            || self.gpio_chip_label != other.gpio_chip_label
            || self.unifont_path != other.unifont_path
    }
}

/// The settings in the store at `path`, and what was wrong with the file
/// or with any value in it.
pub fn load(path: &str) -> (Config, Vec<String>) {
    match Store::load(path) {
        Ok(store) => from_store(&store),
        Err(e) => (Config::default(), vec![e]),
    }
}

/// A day, in seconds: the longest any of the timers is asked to wait.
const DAY: i64 = 86_400;

pub fn from_store(store: &Store) -> (Config, Vec<String>) {
    let mut c = Config::default();
    let mut problems = Vec::new();

    let mut number = |key: &str, min: i64, max: i64| -> Option<i64> {
        match store.integer_in(key, min, max) {
            Ok(v) => v,
            Err(e) => {
                problems.push(e);
                None
            }
        }
    };
    if let Some(v) = number("contrast", 1, 255) {
        c.contrast = v as u8;
    }
    if let Some(v) = number("dim_contrast", 0, 255) {
        c.dim_contrast = v as u8;
    }
    if let Some(v) = number("sleep_after", 0, DAY) {
        c.dim_after = Duration::from_secs(v as u64);
    }
    if let Some(v) = number("deep_sleep_after", 0, DAY) {
        c.blank_after = Duration::from_secs(v as u64);
    }
    if let Some(v) = number("clock_after", 0, DAY) {
        c.clock_after = Duration::from_secs(v as u64);
    }
    if let Some(v) = number("meters_gain", -12, 12) {
        c.meters_gain = v as i8;
    }
    if let Some(v) = number("poll_interval_ms", 100, 10_000) {
        c.poll_interval = Duration::from_millis(v as u64);
    }
    if let Some(v) = number("request_timeout_ms", 50, 10_000) {
        c.request_timeout = Duration::from_millis(v as u64);
    }
    if let Some(v) = number("greeting_hold_ms", 0, 60_000) {
        c.greeting_hold = Duration::from_millis(v as u64);
    }
    if let Some(v) = number("spi_speed_hz", 100_000, 20_000_000) {
        c.spi_speed_hz = v as u32;
    }
    if let Some(v) = number("dc_gpio", 0, 53) {
        c.dc_gpio = v as u32;
    }
    if let Some(v) = number("reset_gpio", 0, 53) {
        c.reset_gpio = v as u32;
    }

    if let Some(v) = store.text("clock_segments") {
        match Unlit::parse(&v) {
            Some(unlit) => c.clock_segments = unlit,
            None => problems.push(format!(
                "clock_segments: not off, very_faint, faint or dim: {v}"
            )),
        }
    }

    let mut place = Place::Under;
    if let Some(v) = store.text("meters_style") {
        match meter_place(&v) {
            Some(p) => place = p,
            None => problems.push(format!(
                "meters_style: not horizontal, horizontal_beside, vertical_left or vertical_right: {v}"
            )),
        }
    }
    // A store written before the time had sizes holds a switch and no
    // size: on is the smallest, which is the size the time then had.
    match store.text("track_time_size") {
        Some(v) => match time_size(&v) {
            Some(size) => c.track_time = size,
            None => problems.push(format!(
                "track_time_size: not off, small, medium, large or extra_large: {v}"
            )),
        },
        None => match store.switch("track_time") {
            Ok(Some(true)) => c.track_time = Some(Size::Small),
            Ok(Some(false)) => c.track_time = None,
            Ok(None) => {}
            Err(e) => problems.push(e),
        },
    }
    match store.switch("snake") {
        Ok(Some(v)) => c.snake = v,
        Ok(None) => {}
        Err(e) => problems.push(e),
    }
    match store.switch("meters") {
        Ok(Some(true)) => c.meters = Some(place),
        Ok(Some(false)) | Ok(None) => c.meters = None,
        Err(e) => problems.push(e),
    }

    match store.switch("remote_active") {
        Ok(Some(v)) => c.remote = v,
        Ok(None) => {}
        Err(e) => problems.push(e),
    }
    if let Some(v) = store.text("log_level") {
        match Level::parse(&v) {
            Some(level) => c.log_level = level,
            None => problems.push(format!("log_level: not error, info or debug: {v}")),
        }
    }

    for (key, field) in [
        ("player_base", &mut c.player_base),
        ("spi_device", &mut c.spi_device),
        ("greeting_text", &mut c.greeting_text),
        ("logo_path", &mut c.logo_path),
        ("farewell_text", &mut c.farewell_text),
        ("restart_text", &mut c.restart_text),
        ("unifont_path", &mut c.unifont_path),
    ] {
        if let Some(v) = store.text(key) {
            *field = v;
        }
    }
    if let Some(v) = store.text("gpio_chip_label") {
        c.gpio_chip_label = Some(v);
    }

    (c, problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_evo_sabre_as_wired() {
        let c = Config::default();
        assert_eq!(c.spi_device, "/dev/spidev0.0");
        assert_eq!(c.spi_speed_hz, 4_800_000);
        assert_eq!(c.dc_gpio, 27);
        assert_eq!(c.reset_gpio, 24);
        assert_eq!(c.gpio_chip_label, None);
        assert_eq!(c.contrast, 100);
        assert_eq!(c.dim_after, Duration::from_secs(1200));
        assert_eq!(c.blank_after, Duration::from_secs(3600));
        assert_eq!(c.clock_after, Duration::from_secs(6));
        assert_eq!(c.clock_segments, Unlit::VeryFaint);
        assert_eq!(c.meters, None);
        assert_eq!(c.meters_gain, 0);
        assert_eq!(c.track_time, Some(Size::Small));
        assert!(!c.snake);
        assert!(c.remote);
        assert_eq!(c.log_level, Level::Error);
        assert!(c
            .logo_path
            .ends_with("/audiophonics_evo_sabre/assets/logo.pgm"));
        assert!(c
            .unifont_path
            .ends_with("/audiophonics_evo_sabre/assets/unifont.hex"));
    }

    #[test]
    fn the_settings_page_keys_are_read_from_the_store() {
        let store = Store::parse(
            r#"{
            "oled_active": { "type": "boolean", "value": true },
            "remote_active": { "type": "boolean", "value": false },
            "contrast": { "type": "number", "value": 200 },
            "sleep_after": { "type": "number", "value": 300 },
            "deep_sleep_after": { "type": "number", "value": 0 },
            "clock_after": { "type": "number", "value": "30" },
            "clock_segments": { "type": "string", "value": "off" },
            "meters": { "type": "boolean", "value": true },
            "meters_style": { "type": "string", "value": "vertical_right" },
            "meters_gain": { "type": "number", "value": -6 },
            "track_time": { "type": "boolean", "value": true },
            "track_time_size": { "type": "string", "value": "extra_large" },
            "snake": { "type": "boolean", "value": true },
            "log_level": { "type": "string", "value": "info" }
        }"#,
        )
        .unwrap();
        let (c, problems) = from_store(&store);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(!c.remote);
        assert_eq!(c.contrast, 200);
        assert_eq!(c.dim_after, Duration::from_secs(300));
        assert_eq!(c.blank_after, Duration::ZERO);
        assert_eq!(c.clock_after, Duration::from_secs(30));
        assert_eq!(c.clock_segments, Unlit::Off);
        assert_eq!(c.meters, Some(Place::Right));
        assert_eq!(c.meters_gain, -6);
        assert_eq!(c.track_time, Some(Size::ExtraLarge));
        assert!(c.snake);
        assert_eq!(c.log_level, Level::Info);
        assert_eq!(c.dc_gpio, 27, "a key that is absent leaves its default");
    }

    #[test]
    fn a_value_that_cannot_be_used_is_named_and_the_default_stays() {
        let store = Store::parse(
            r#"{
            "contrast": { "type": "number", "value": 0 },
            "sleep_after": { "type": "number", "value": "soon" },
            "remote_active": { "type": "boolean", "value": "perhaps" },
            "log_level": { "type": "string", "value": "loud" },
            "dc_gpio": { "type": "number", "value": 23 }
        }"#,
        )
        .unwrap();
        let (c, problems) = from_store(&store);
        assert_eq!(problems.len(), 4, "{problems:?}");
        assert!(problems.iter().any(|p| p.starts_with("contrast:")));
        assert!(problems.iter().any(|p| p.starts_with("sleep_after:")));
        assert!(problems.iter().any(|p| p.starts_with("remote_active:")));
        assert!(problems.iter().any(|p| p.starts_with("log_level:")));
        assert_eq!(c.contrast, 100);
        assert_eq!(c.dim_after, Duration::from_secs(1200));
        assert!(c.remote);
        assert_eq!(c.dc_gpio, 23, "the wiring keys are read too");
    }

    #[test]
    fn a_missing_store_is_the_defaults_and_a_damaged_one_says_so() {
        let (c, problems) = load("/nonexistent/evosabre/config.json");
        assert_eq!(c, Config::default());
        assert!(problems.is_empty());
        let path = std::env::temp_dir().join(format!("evosabre-config-{}", std::process::id()));
        std::fs::write(&path, "{ nope").unwrap();
        let (c, problems) = load(path.to_str().unwrap());
        assert_eq!(c, Config::default());
        assert_eq!(problems.len(), 1);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn the_dim_level_is_never_brighter_than_the_full_one() {
        let mut c = Config::default();
        assert_eq!(c.dimmed(), 24);
        c.contrast = 10;
        assert_eq!(c.dimmed(), 10);
    }

    #[test]
    fn a_change_of_wiring_is_told_from_a_change_of_taste() {
        let a = Config::default();
        let mut b = a.clone();
        b.contrast = 5;
        b.clock_after = Duration::ZERO;
        assert!(!a.rewired(&b));
        b.dc_gpio = 23;
        assert!(a.rewired(&b));
    }

    #[test]
    fn the_meters_are_off_unless_switched_on_and_lie_under_the_track_unless_placed() {
        let on = Store::parse(r#"{ "meters": { "type": "boolean", "value": true } }"#).unwrap();
        assert_eq!(from_store(&on).0.meters, Some(Place::Under));

        let placed_but_off = Store::parse(
            r#"{ "meters": { "type": "boolean", "value": false },
                 "meters_style": { "type": "string", "value": "vertical_left" } }"#,
        )
        .unwrap();
        assert_eq!(from_store(&placed_but_off).0.meters, None);

        let odd = Store::parse(
            r#"{ "meters": { "type": "boolean", "value": true },
                 "meters_style": { "type": "string", "value": "diagonal" } }"#,
        )
        .unwrap();
        let (c, problems) = from_store(&odd);
        assert_eq!(
            c.meters,
            Some(Place::Under),
            "a place not known leaves the first"
        );
        assert!(problems.iter().any(|p| p.starts_with("meters_style:")));

        let loud = Store::parse(r#"{ "meters_gain": { "type": "number", "value": 20 } }"#).unwrap();
        let (c, problems) = from_store(&loud);
        assert_eq!(c.meters_gain, 0, "a gain out of range leaves none");
        assert!(problems.iter().any(|p| p.starts_with("meters_gain:")));

        let beside = Store::parse(
            r#"{ "meters": { "type": "boolean", "value": true },
                 "meters_style": { "type": "string", "value": "horizontal_beside" } }"#,
        )
        .unwrap();
        assert_eq!(from_store(&beside).0.meters, Some(Place::Between));
    }

    fn time_of(store: &str) -> (Option<Size>, Vec<String>) {
        let (c, problems) = from_store(&Store::parse(store).unwrap());
        (c.track_time, problems)
    }

    #[test]
    fn the_time_is_the_size_asked_for_and_off_leaves_it_out() {
        for (word, size) in [
            ("small", Some(Size::Small)),
            ("medium", Some(Size::Medium)),
            ("large", Some(Size::Large)),
            ("extra_large", Some(Size::ExtraLarge)),
            ("off", None),
        ] {
            let store =
                format!(r#"{{ "track_time_size": {{ "type": "string", "value": "{word}" }} }}"#);
            assert_eq!(time_of(&store), (size, vec![]), "{word}");
        }
        let (size, problems) =
            time_of(r#"{ "track_time_size": { "type": "string", "value": "huge" } }"#);
        assert_eq!(size, Some(Size::Small), "a size not known leaves the first");
        assert!(problems.iter().any(|p| p.starts_with("track_time_size:")));
    }

    #[test]
    fn a_store_from_before_the_sizes_keeps_its_time_as_it_was() {
        // The switch alone: on is the size the time then had, off is off.
        assert_eq!(
            time_of(r#"{ "track_time": { "type": "boolean", "value": true } }"#).0,
            Some(Size::Small)
        );
        assert_eq!(
            time_of(r#"{ "track_time": { "type": "boolean", "value": false } }"#).0,
            None
        );
        // Once a size is stored it is the word, whatever the switch says.
        assert_eq!(
            time_of(
                r#"{ "track_time": { "type": "boolean", "value": false },
                     "track_time_size": { "type": "string", "value": "large" } }"#
            )
            .0,
            Some(Size::Large)
        );
        assert_eq!(
            time_of(
                r#"{ "track_time": { "type": "boolean", "value": true },
                     "track_time_size": { "type": "string", "value": "off" } }"#
            )
            .0,
            None
        );
    }
}
