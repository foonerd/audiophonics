//! The panel's settings.
//!
//! The defaults are the RaspDac Mini as it is wired: the LCD on SPI0 CE0
//! with D/C on GPIO27 and reset on GPIO24. The plugin's settings page keeps
//! the user's choices in Volumio's configuration store, and this reads that
//! file; the wiring and the words are read from the same file when a key
//! for them is there, for a board wired another way.
//!
//! A key that is absent leaves its default. A value that cannot be used is
//! named and leaves its default too: the panel comes up on a damaged store
//! and says in the journal what was wrong with it.

use crate::ili9341::Wiring;
use audiophonics_base::log::Level;
use audiophonics_base::settings::Store;
use std::time::Duration;

pub const DEFAULT_PATH: &str = "/data/configuration/system_hardware/raspdac_mini_lcd/config.json";
const PLUGIN_DIR: &str = "/data/plugins/system_hardware/raspdac_mini_lcd";

/// The longest the screen can be asked to stay up while nothing plays.
const HOUR: i64 = 3600;

/// How the panel is driven: see `panel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    /// Through the kernel's driver, as a framebuffer.
    Framebuffer,
    /// By this program, over SPI.
    Spi,
}

impl Driver {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "framebuffer" => Some(Self::Framebuffer),
            "spi" => Some(Self::Spi),
            _ => None,
        }
    }
}

/// Where the level meters stand when they are on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Upright, at the right of the cover.
    Right,
    /// Upright, at the left of the cover.
    Left,
    /// Lying, over the track's line.
    Over,
}

impl Place {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "vertical_right" => Some(Self::Right),
            "vertical_left" => Some(Self::Left),
            "horizontal" => Some(Self::Over),
            _ => None,
        }
    }

    /// True for meters that stand upright beside the cover. They change
    /// nearly every row of the glass with every frame, which is more than
    /// can be sent to the panel fifty times a second.
    pub fn upright(self) -> bool {
        matches!(self, Self::Right | Self::Left)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub player_base: String,
    pub poll_interval: Duration,
    pub request_timeout: Duration,

    pub driver: Driver,
    /// How the panel is wired, for when this program drives it.
    pub wiring: Wiring,
    /// Where the faces and the mark are.
    pub assets: String,

    /// Whether the mark is shown while the player starts and when it
    /// leaves. Without it the screen stays black until there is a track to
    /// show, and goes black when the player goes.
    pub splash: bool,
    pub starting_text: String,
    pub farewell_text: String,
    pub restart_text: String,

    /// The screen goes black after this long while nothing plays and
    /// nothing is touched. Zero keeps it up for ever.
    pub sleep_after: Duration,

    /// The level meters: off, or where they stand on the first page.
    pub meters: Option<Place>,
    /// Decibels added to what the meters show, -12 to 12.
    pub meters_gain: i8,

    /// Whether the handset is obeyed.
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

            driver: Driver::Framebuffer,
            wiring: Wiring {
                spi_device: "/dev/spidev0.0".into(),
                spi_speed_hz: 64_000_000,
                dc_gpio: 27,
                reset_gpio: 24,
                gpio_chip_label: None,
            },
            assets: format!("{PLUGIN_DIR}/assets"),

            splash: true,
            starting_text: "Starting".into(),
            farewell_text: "Shutting down".into(),
            restart_text: "Restarting".into(),

            sleep_after: Duration::from_secs(900),

            meters: None,
            meters_gain: 0,

            remote: true,
            log_level: Level::Error,
        }
    }
}

impl Config {
    /// True when what differs from `other` is only read at start: how the
    /// panel is driven and wired, and where its faces are.
    pub fn rewired(&self, other: &Config) -> bool {
        self.driver != other.driver || self.wiring != other.wiring || self.assets != other.assets
    }
}

/// The settings from the store at `path`, and what was wrong with it.
pub fn load(path: &str) -> (Config, Vec<String>) {
    match Store::load(path) {
        Ok(store) => from_store(&store),
        Err(e) => (Config::default(), vec![e]),
    }
}

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
    if let Some(v) = number("sleep_after", 0, HOUR) {
        c.sleep_after = Duration::from_secs(v as u64);
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
    if let Some(v) = number("spi_speed_hz", 1_000_000, 80_000_000) {
        c.wiring.spi_speed_hz = v as u32;
    }
    if let Some(v) = number("dc_gpio", 0, 53) {
        c.wiring.dc_gpio = v as u32;
    }
    if let Some(v) = number("reset_gpio", 0, 53) {
        c.wiring.reset_gpio = v as u32;
    }

    for (key, field) in [
        ("boot_splash", &mut c.splash),
        ("remote_active", &mut c.remote),
    ] {
        match store.switch(key) {
            Ok(Some(v)) => *field = v,
            Ok(None) => {}
            Err(e) => problems.push(e),
        }
    }
    if let Some(v) = store.text("display_driver") {
        match Driver::parse(&v) {
            Some(driver) => c.driver = driver,
            None => problems.push(format!("display_driver: not framebuffer or spi: {v}")),
        }
    }
    // The meters stand at the right unless the store places them, and are
    // there only when it has them switched on.
    let mut place = Place::Right;
    if let Some(v) = store.text("meters_style") {
        match Place::parse(&v) {
            Some(named) => place = named,
            None => problems.push(format!(
                "meters_style: not vertical_right, vertical_left or horizontal: {v}"
            )),
        }
    }
    match store.switch("meters") {
        Ok(Some(true)) => c.meters = Some(place),
        Ok(Some(false)) | Ok(None) => c.meters = None,
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
        ("spi_device", &mut c.wiring.spi_device),
        ("assets", &mut c.assets),
        ("starting_text", &mut c.starting_text),
        ("farewell_text", &mut c.farewell_text),
        ("restart_text", &mut c.restart_text),
    ] {
        if let Some(v) = store.text(key) {
            *field = v;
        }
    }
    if let Some(v) = store.text("gpio_chip_label") {
        c.wiring.gpio_chip_label = Some(v);
    }

    (c, problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_raspdac_mini_as_wired() {
        let c = Config::default();
        assert_eq!(c.driver, Driver::Framebuffer);
        assert_eq!(c.wiring.spi_device, "/dev/spidev0.0");
        assert_eq!(c.wiring.spi_speed_hz, 64_000_000);
        assert_eq!((c.wiring.dc_gpio, c.wiring.reset_gpio), (27, 24));
        assert_eq!(c.wiring.gpio_chip_label, None);
        assert_eq!(c.sleep_after, Duration::from_secs(900));
        assert!(c.splash && c.remote);
        assert_eq!(c.meters, None);
        assert_eq!(c.meters_gain, 0);
        assert_eq!(c.log_level, Level::Error);
        assert_eq!(
            c.assets,
            "/data/plugins/system_hardware/raspdac_mini_lcd/assets"
        );
    }

    #[test]
    fn the_settings_page_keys_are_read_from_the_store() {
        let store = Store::parse(
            r#"{
                "lcd_active": { "type": "boolean", "value": true },
                "boot_splash": { "type": "boolean", "value": false },
                "sleep_after": { "type": "number", "value": 0 },
                "remote_active": { "type": "boolean", "value": false },
                "display_driver": { "type": "string", "value": "spi" },
                "log_level": { "type": "string", "value": "info" }
            }"#,
        )
        .unwrap();
        let (c, problems) = from_store(&store);
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!(c.driver, Driver::Spi);
        assert!(!c.splash);
        assert_eq!(c.sleep_after, Duration::ZERO, "nought is never");
        assert!(!c.remote);
        assert_eq!(c.log_level, Level::Info);
    }

    #[test]
    fn the_settings_version_1_left_are_taken_as_they_are() {
        // What 1.2.0 keeps: its three keys and nothing else.
        let store = Store::parse(
            r#"{
                "lcd_active": { "type": "boolean", "value": true },
                "sleep_after": { "type": "number", "value": 1800 },
                "boot_splash": { "type": "boolean", "value": true }
            }"#,
        )
        .unwrap();
        let (c, problems) = from_store(&store);
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!(c.sleep_after, Duration::from_secs(1800));
        assert!(c.splash && c.remote);
        assert_eq!(c.driver, Driver::Framebuffer, "as version 1 drove it");
    }

    #[test]
    fn a_value_that_cannot_be_used_is_named_and_the_default_stays() {
        let store = Store::parse(
            r#"{
                "sleep_after": { "type": "number", "value": 7200 },
                "boot_splash": { "type": "string", "value": "perhaps" },
                "log_level": { "type": "string", "value": "loud" },
                "display_driver": { "type": "string", "value": "hdmi" },
                "spi_speed_hz": { "type": "number", "value": 5 }
            }"#,
        )
        .unwrap();
        let (c, problems) = from_store(&store);
        assert_eq!(c, Config::default());
        assert_eq!(problems.len(), 5, "{problems:?}");
        for key in [
            "sleep_after",
            "boot_splash",
            "log_level",
            "display_driver",
            "spi_speed_hz",
        ] {
            assert!(
                problems.iter().any(|p| p.contains(key)),
                "{key}: {problems:?}"
            );
        }
    }

    #[test]
    fn the_meters_are_off_unless_switched_on_and_stand_at_the_right_unless_placed() {
        let parsed = |json: &str| from_store(&Store::parse(json).unwrap());
        let (c, problems) = parsed(r#"{ "meters": { "type": "boolean", "value": true } }"#);
        assert_eq!(problems, Vec::<String>::new());
        assert_eq!(c.meters, Some(Place::Right));
        assert_eq!(c.meters_gain, 0);

        for (style, place) in [
            ("vertical_right", Place::Right),
            ("vertical_left", Place::Left),
            ("horizontal", Place::Over),
        ] {
            let (c, problems) = parsed(&format!(
                r#"{{ "meters": {{ "type": "boolean", "value": true }},
                     "meters_style": {{ "type": "string", "value": "{style}" }},
                     "meters_gain": {{ "type": "number", "value": -6 }} }}"#
            ));
            assert_eq!(problems, Vec::<String>::new());
            assert_eq!(c.meters, Some(place));
            assert_eq!(c.meters_gain, -6);
        }
        assert!(Place::Right.upright() && Place::Left.upright() && !Place::Over.upright());

        // Placed but not switched on: off.
        let (c, _) = parsed(r#"{ "meters_style": { "type": "string", "value": "horizontal" } }"#);
        assert_eq!(c.meters, None);

        // A place that is none, and a gain out of range, are named; the
        // meters still come on, where they stand by default.
        let (c, problems) = parsed(
            r#"{ "meters": { "type": "boolean", "value": true },
                 "meters_style": { "type": "string", "value": "diagonal" },
                 "meters_gain": { "type": "number", "value": 40 } }"#,
        );
        assert_eq!(c.meters, Some(Place::Right));
        assert_eq!(c.meters_gain, 0);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("meters_style")));
        assert!(problems.iter().any(|p| p.contains("meters_gain")));
    }

    #[test]
    fn a_missing_store_is_the_defaults_and_a_damaged_one_says_so() {
        let (c, problems) = load("/nonexistent/config.json");
        assert_eq!(c, Config::default());
        assert!(problems.is_empty() || problems[0].contains("/nonexistent"));
        let path = std::env::temp_dir().join(format!("raspdacmini-config-{}", std::process::id()));
        std::fs::write(&path, "{ not json").unwrap();
        let (c, problems) = load(path.to_str().unwrap());
        assert_eq!(c, Config::default());
        assert_eq!(problems.len(), 1);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_change_of_wiring_is_told_from_a_change_of_taste() {
        let base = Config::default();
        let mut taste = base.clone();
        taste.sleep_after = Duration::ZERO;
        taste.remote = false;
        assert!(!base.rewired(&taste));
        let mut wired = base.clone();
        wired.wiring.dc_gpio = 25;
        assert!(base.rewired(&wired));
        let mut moved = base.clone();
        moved.assets = "/tmp/assets".into();
        assert!(base.rewired(&moved));
        let mut driven = base.clone();
        driven.driver = Driver::Spi;
        assert!(base.rewired(&driven));
    }
}
