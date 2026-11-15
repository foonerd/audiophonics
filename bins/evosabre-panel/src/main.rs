//! evosabre-panel: the EVO Sabre's second display and its remote.
//!
//! Start: open the display, show the greeting, find the infrared receiver.
//! Then, for as long as the process lives: poll the player at the panel's
//! interval, take a button the moment it arrives, and redraw when either
//! changed anything. Every change of player state, every button and every
//! outage goes to the journal once, so the unit can be checked from a
//! terminal as well as from the glass.
//!
//! Brightness follows use: full after a button or when playback starts, dim
//! after `dim_after`, dark after `blank_after` while nothing plays. On
//! SIGTERM the panel is blanked before the process leaves, so a stopped
//! service is a dark panel, not a frozen page; unless the player is going
//! down, which the plugin announces by signal, in which case the panel says
//! goodbye first; see `farewell`. SIGHUP reads the settings again.
//!
//! A player that does not answer is not an error: Volumio restarts, and the
//! panel outlives it. The process exits only for a display it cannot open
//! or a bus write that fails, each said plainly, each retried by systemd
//! within its start limit. A receiver that is not there is reported once and
//! looked for again every few seconds, since at boot the panel may be up
//! before it; the panel carries on without the remote meanwhile.

mod config;
mod frame;
mod greeting;
mod grey;
mod icons;
mod keys;
mod layout;
mod orbit;
mod render;
mod scroll;
mod segments;
mod ssd1322;
mod text;

use audiophonics_base::clock::Clock;
use audiophonics_base::idle::{Idle, Level, Playing};
use audiophonics_base::{board, debug, error, info, leaving as farewell, log, net};
use audiophonics_meters::{self as meters, levels};
use audiophonics_player::probe::{Probe, Report};
use audiophonics_player::{Player, Volumio};
use audiophonics_remote::watch::{self, Watch};
use audiophonics_remote::{Event, Press, Status};
use audiophonics_unifont::Unifont;
use config::Config;
use frame::Frame;
use keys::Action;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

const NAME: &str = "evosabre-panel";
/// The tag the plugin's ALSA contribution gives its tap, and so the one in
/// the names of the rings the meters read.
const TAP_TAG: &str = "evosabre";

const USAGE: &str = "\
usage: evosabre-panel [--config FILE]
       evosabre-panel --render DIR [--assets DIR]
       evosabre-panel --levels
       evosabre-panel --version

  --config FILE  the plugin's settings, as Volumio's configuration store
                 keeps them; without it, the installed plugin's own
  --render DIR   draw every screen into DIR as PGM images, and leave
  --assets DIR   with --render: the logo and the Unifont file to draw with
  --levels       say for ten seconds what the level meters read from the
                 tap in the player's audio path, and leave
  --version      say which build this is";

/// Redraw rate while a line scrolls: one pixel per frame at the marquee's
/// speed.
const SCROLL_FRAME: Duration = Duration::from_millis(1000 / scroll::SPEED as u64);
/// A frame of the meters: twenty-five a second, from the start of one to
/// the start of the next.
const METER_FRAME: Duration = Duration::from_millis(40);
/// How often the address is looked up again, in polls.
const ADDRESS_EVERY: u32 = 10;
/// How often the time zone is read again, in polls: it may be changed while
/// the panel runs.
const ZONE_EVERY: u32 = 120;
/// How long after the player last played it still counts as playing, for
/// the burn-in policy: the player reports a stop between two tracks.
const REST_GRACE: Duration = Duration::from_secs(10);

/// What the loop wakes for besides time.
enum Msg {
    Remote(Event),
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Run { config: String },
    Render { dir: String, assets: Option<String> },
    Levels,
    Version,
    Help,
}

fn parse(args: &[String]) -> Result<Command, String> {
    let mut config = config::DEFAULT_PATH.to_string();
    let mut render = None;
    let mut assets = None;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        let mut value = |what: &str| {
            rest.next()
                .cloned()
                .ok_or_else(|| format!("{arg} needs {what}"))
        };
        match arg.as_str() {
            "--version" | "-V" => return Ok(Command::Version),
            "--help" | "-h" => return Ok(Command::Help),
            "--levels" => return Ok(Command::Levels),
            "--config" => config = value("a file")?,
            "--render" => render = Some(value("a directory")?),
            "--assets" => assets = Some(value("a directory")?),
            other => return Err(format!("{other}: not an option")),
        }
    }
    match (render, assets) {
        (Some(dir), assets) => Ok(Command::Render { dir, assets }),
        (None, Some(_)) => Err("--assets goes with --render".to_string()),
        (None, None) => Ok(Command::Run { config }),
    }
}

fn main() -> ExitCode {
    log::name_as(NAME);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let done = match parse(&args) {
        Ok(Command::Version) => {
            println!("{NAME} {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Ok(Command::Help) => {
            println!("{USAGE}");
            Ok(())
        }
        Ok(Command::Render { dir, assets }) => render_all(&dir, assets.as_deref()),
        Ok(Command::Levels) => say_levels(),
        Ok(Command::Run { config }) => run(&config),
        Err(e) => {
            eprintln!("{NAME}: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match done {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{NAME}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// What the meters would show, said four times a second for ten seconds:
/// the tap's ring as the display reads it, for a look without the glass.
fn say_levels() -> Result<(), String> {
    let mut source = levels::Source::new(TAP_TAG);
    let mut needles = meters::Meters::default();
    let started = Instant::now();
    let mut said = started;
    let mut heard_any = false;
    while started.elapsed() < Duration::from_secs(10) {
        let now = Instant::now();
        let heard = source.take(now);
        heard_any |= heard.is_some();
        needles.feed(heard, now);
        match source.changed() {
            Some(true) => println!(
                "reading {}",
                source
                    .path()
                    .map_or_else(String::new, |p| p.display().to_string())
            ),
            Some(false) => println!("no stream to read"),
            None => {}
        }
        if now.saturating_duration_since(said) >= Duration::from_millis(250) {
            said = now;
            let [left, right] = needles.bars();
            let db = |place: f32| meters::FLOOR_DB * (1.0 - place);
            println!(
                "left level {:6.1} dB peak {:6.1} dB hold {:6.1} dB   right level {:6.1} dB peak {:6.1} dB hold {:6.1} dB",
                db(left.level),
                db(left.peak),
                db(left.hold),
                db(right.level),
                db(right.peak),
                db(right.hold)
            );
        }
        std::thread::sleep(METER_FRAME);
    }
    if heard_any {
        Ok(())
    } else {
        Err(format!(
            "nothing was published under {} for the tag {} in ten seconds: \
             nothing is playing, or the tap is not in the player's audio path",
            levels::DIR,
            TAP_TAG
        ))
    }
}

fn render_all(dir: &str, assets: Option<&str>) -> Result<(), String> {
    let mut cfg = Config::default();
    if let Some(assets) = assets {
        cfg.logo_path = format!("{assets}/logo.pgm");
        cfg.unifont_path = format!("{assets}/unifont.hex");
    }
    for file in render::all(Path::new(dir), &cfg)? {
        println!("{}", file.display());
    }
    Ok(())
}

/// A flag one or more signals raise.
fn raised_by(signals: &[i32]) -> Result<Arc<AtomicBool>, String> {
    let flag = Arc::new(AtomicBool::new(false));
    for &signal in signals {
        signal_hook::flag::register(signal, Arc::clone(&flag))
            .map_err(|e| format!("signal handler: {e}"))?;
    }
    Ok(flag)
}

/// A frame from the handset: said in the journal, noted for the settings
/// page, and obeyed where it is one of the keys the streamer takes. Returns
/// whether the player was asked for something.
fn obey(press: &Press, player: &Volumio, status: &mut Status) -> bool {
    let fresh = press.repeat == 0;
    match press.key {
        Some(key) if fresh => {
            info!("remote: {key}");
            status.key(key.to_string());
        }
        Some(key) => debug!("remote: {key}, repeat {}", press.repeat),
        None if fresh => {
            info!(
                "remote: scancode {:#x} is not one of the handset's",
                press.scancode
            );
            status.key(format!("scancode {:#x}", press.scancode));
        }
        None => {}
    }
    let Some(action) = keys::action(press) else {
        return false;
    };
    let asked = match action {
        Action::Transport(cmd) => player.transport(cmd),
        Action::Seek(step) => player.seek(step),
    };
    match asked {
        Ok(()) => {
            debug!("remote: asked the player for {action}");
            true
        }
        Err(e) => {
            error!("remote: {action}: {e}");
            false
        }
    }
}

fn run(path: &str) -> Result<(), String> {
    let (mut cfg, problems) = config::load(path);
    log::set(cfg.log_level);
    // The one line always said: which build, and how loud it will be.
    println!(
        "{NAME} {} watching {} every {} ms, log level {}",
        env!("CARGO_PKG_VERSION"),
        cfg.player_base,
        cfg.poll_interval.as_millis(),
        cfg.log_level
    );
    for problem in &problems {
        error!("settings: {problem}");
    }
    info!("board: {}", board::detect());

    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM, SIGUSR1, SIGUSR2};
    let stop = raised_by(&[SIGTERM, SIGINT])?;
    let reload = raised_by(&[SIGHUP])?;
    // The plugin's word that the player is going down: USR1 a halt, USR2 a
    // restart.
    let going_off = raised_by(&[SIGUSR1])?;
    let going_restart = raised_by(&[SIGUSR2])?;
    let mut leaving: Option<farewell::Leaving> = None;

    let display = |e: String| format!("display: {e}");
    let mut panel = ssd1322::Ssd1322::open(&cfg).map_err(display)?;
    info!("display: {}", panel.describe());

    let (frame, source) = greeting::load(&cfg);
    info!("greeting: {source}");
    panel.flush(&frame).map_err(display)?;
    let greeted_at = Instant::now();
    let mut shown = frame;

    let (tx, rx) = mpsc::channel::<Msg>();
    let mut status = Status::new();
    let mut remote = Watch::new(greeted_at);
    if !cfg.remote {
        status.receiver(watch::OFF);
    }

    let mut volumio = Volumio::new(cfg.player_base.clone(), cfg.request_timeout);
    let mut probe = Probe::default();
    let mut screen = layout::Playing::default();
    let mut selector = layout::Selector::default();
    let mut source = levels::Source::new(TAP_TAG);
    let mut needles = meters::Meters::default();
    let mut metering = false;
    let mut drawn_at = Instant::now();
    let mut font = Unifont::new(&cfg.unifont_path);
    let (mut clock, warning) = Clock::new();
    if let Some(w) = warning {
        error!("{w}");
    }
    let mut ip = net::address().unwrap_or_else(|| "no network".into());
    let mut polls: u32 = 0;
    let mut idle = Idle::new(greeted_at, cfg.dim_after, cfg.blank_after);
    let mut steady = Playing::new(REST_GRACE);
    let mut level = Level::Full;
    let mut greeting_up = true;
    let mut moving = false;
    let mut next_poll = Instant::now();

    loop {
        // The settings page saved something: read the store again. What is
        // only read at start, the wiring, waits for the next start.
        if reload.swap(false, Ordering::Relaxed) {
            let (new, problems) = config::load(path);
            for problem in &problems {
                error!("settings: {problem}");
            }
            log::set(new.log_level);
            if cfg.rewired(&new) {
                error!(
                    "settings: the wiring changed; it takes effect when the service next starts"
                );
            }
            idle.retime(new.dim_after, new.blank_after);
            volumio = Volumio::new(new.player_base.clone(), new.request_timeout);
            match level {
                Level::Full => panel.set_contrast(new.contrast).map_err(display)?,
                Level::Dim => panel.set_contrast(new.dimmed()).map_err(display)?,
                Level::Blank => {}
            }
            if new.remote {
                remote.look_now(Instant::now());
                status.receiver(remote.line());
            } else {
                status.receiver(watch::OFF);
            }
            (clock, _) = Clock::new();
            cfg = new;
            info!("settings read again");
        }

        // The player is going down: say so, at full brightness, in place of
        // whatever was up.
        let announced = if going_off.swap(false, Ordering::Relaxed) {
            Some(farewell::Leaving::Off)
        } else if going_restart.swap(false, Ordering::Relaxed) {
            Some(farewell::Leaving::Restart)
        } else {
            None
        };
        if let Some(way) = announced {
            if leaving.is_none() {
                let text = match way {
                    farewell::Leaving::Off => &cfg.farewell_text,
                    farewell::Leaving::Restart => &cfg.restart_text,
                };
                info!(
                    "leaving: {}",
                    match way {
                        farewell::Leaving::Off => "power off",
                        farewell::Leaving::Restart => "restart",
                    }
                );
                if level == Level::Blank {
                    panel.wake().map_err(display)?;
                }
                panel.set_contrast(cfg.contrast).map_err(display)?;
                level = Level::Full;
                let frame = greeting::text(text);
                panel.flush(&frame).map_err(display)?;
                shown = frame;
                leaving = Some(way);
            }
        }
        if stop.load(Ordering::Relaxed) {
            match farewell::at_stop(leaving) {
                farewell::AtStop::Blank => {
                    panel.sleep().map_err(display)?;
                    info!("stopping: panel blanked");
                }
                farewell::AtStop::Keep => {
                    info!("stopping: farewell left on the glass for the greeting to replace");
                }
                farewell::AtStop::HoldThenBlank => {
                    std::thread::sleep(farewell::HOLD);
                    panel.sleep().map_err(display)?;
                    info!("stopping: goodbye said, panel blanked");
                }
            }
            return Ok(());
        }

        if leaving.is_some() {
            // Nothing to draw and nothing to obey; wait for the stop.
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    std::thread::sleep(Duration::from_millis(200))
                }
            }
            continue;
        }

        let now = Instant::now();
        if cfg.remote && remote.due(now) {
            remote.look(&tx, Msg::Remote, now);
            status.receiver(remote.line());
        }
        if now >= next_poll {
            match probe.observe(volumio.state()) {
                Some(Report::Changed(line)) => info!("{line}"),
                Some(Report::Outage(line)) => error!("{line}"),
                None => {}
            }
            let playing = probe.state().is_some_and(|s| s.is_playing());
            idle.player(now, steady.observe(now, playing));
            polls += 1;
            if polls.is_multiple_of(ADDRESS_EVERY) {
                ip = net::address().unwrap_or_else(|| "no network".into());
            }
            if polls.is_multiple_of(ZONE_EVERY) {
                (clock, _) = Clock::new();
            }
            next_poll = now + cfg.poll_interval;
        }

        let wanted = idle.level(Instant::now());
        if wanted != level {
            if level == Level::Blank {
                panel.wake().map_err(display)?;
                // The page may have moved on while the panel was dark.
                shown = Frame::new();
            }
            match wanted {
                Level::Full => panel.set_contrast(cfg.contrast).map_err(display)?,
                Level::Dim => panel.set_contrast(cfg.dimmed()).map_err(display)?,
                Level::Blank => panel.sleep().map_err(display)?,
            }
            info!("panel {wanted}");
            level = wanted;
        }

        if greeting_up && probe.state().is_some() && greeted_at.elapsed() >= cfg.greeting_hold {
            greeting_up = false;
        }
        if !greeting_up && level != Level::Blank {
            let mut state = probe.state().cloned().unwrap_or_default();
            if probe.state().is_some() && state.status.is_empty() {
                // The player answered without a status, as it does for a
                // moment while it changes track: that is stopped, not gone.
                state.status = "stop".into();
            }
            let mut frame = Frame::new();
            let (dx, dy) = orbit::offset(greeted_at.elapsed());
            frame.set_origin(dx, dy);
            let now = Instant::now();
            let time = match selector.screen(now, &state, cfg.clock_after) {
                layout::Screen::Playing => None,
                layout::Screen::Clock => clock.now(),
            };
            drawn_at = now;
            screen.show_time(cfg.track_time);
            match &time {
                Some(t) => {
                    layout::clock(&mut frame, &state, &ip, t, cfg.clock_segments.level());
                    metering = false;
                }
                None if cfg.meters.is_some() => {
                    let place = cfg.meters.unwrap_or(layout::Place::Under);
                    let gain = f32::from(cfg.meters_gain);
                    needles.feed(source.take(now).map(|level| level.gained(gain)), now);
                    match source.changed() {
                        Some(true) => info!(
                            "meters: reading {}",
                            source
                                .path()
                                .map_or_else(String::new, |p| p.display().to_string())
                        ),
                        Some(false) => info!("meters: no stream to read"),
                        None => {}
                    }
                    let bars = needles.bars();
                    let meters = layout::Metered { bars: &bars, place };
                    screen.render(&mut frame, &state, &ip, now, &mut font, Some(meters));
                    // Fast frames while there is sound to show, and until
                    // the marks have fallen after it.
                    metering = state.is_playing() || !needles.at_rest();
                }
                None => {
                    screen.render(&mut frame, &state, &ip, now, &mut font, None);
                    metering = false;
                }
            }
            moving = time.is_none() && screen.scrolling();
            if let Some(problem) = font.take_problem() {
                error!("text: {problem}; lines beyond Latin-1 are drawn in the panel's own faces");
            }
            if frame != shown {
                panel.flush(&frame).map_err(display)?;
                shown = frame;
            }
        }

        // Sleep until the next poll, or the next scroll frame, or until a
        // button arrives.
        let mut wait = next_poll.saturating_duration_since(Instant::now());
        if moving && !greeting_up && level != Level::Blank {
            wait = wait.min(SCROLL_FRAME);
        }
        if metering && !greeting_up && level != Level::Blank {
            wait = wait.min(METER_FRAME.saturating_sub(drawn_at.elapsed()));
        }
        match rx.recv_timeout(wait) {
            Ok(first) => {
                let now = Instant::now();
                let mut batch = vec![first];
                while let Ok(m) = rx.try_recv() {
                    batch.push(m);
                }
                for msg in batch {
                    match msg {
                        Msg::Remote(Event::Press(press)) => {
                            if !cfg.remote {
                                continue;
                            }
                            // The handset's own buttons are a touch, and
                            // wake a dark panel; another handset's are not.
                            if press.key.is_some() {
                                idle.activity(now);
                            }
                            if obey(&press, &volumio, &mut status) {
                                next_poll = now;
                            }
                        }
                        Msg::Remote(Event::Lost(why)) => {
                            remote.lost(&why, now);
                            status.receiver(remote.line());
                        }
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            // Unreachable while `tx` lives in this function, but the loop
            // must not spin if it ever happens.
            Err(mpsc::RecvTimeoutError::Disconnected) => std::thread::sleep(wait),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn without_arguments_the_panel_runs_on_the_installed_plugins_settings() {
        assert_eq!(
            parse(&[]),
            Ok(Command::Run {
                config: config::DEFAULT_PATH.to_string()
            })
        );
        assert_eq!(
            parse(&args(&["--config", "/tmp/c.json"])),
            Ok(Command::Run {
                config: "/tmp/c.json".into()
            })
        );
    }

    #[test]
    fn the_other_commands_and_a_wrong_one() {
        assert_eq!(parse(&args(&["--version"])), Ok(Command::Version));
        assert_eq!(parse(&args(&["--help"])), Ok(Command::Help));
        assert_eq!(parse(&args(&["--levels"])), Ok(Command::Levels));
        assert_eq!(
            parse(&args(&["--render", "out", "--assets", "a"])),
            Ok(Command::Render {
                dir: "out".into(),
                assets: Some("a".into())
            })
        );
        assert!(parse(&args(&["--render"])).unwrap_err().contains("needs"));
        assert!(parse(&args(&["--assets", "a"]))
            .unwrap_err()
            .contains("--render"));
        assert!(parse(&args(&["--frobnicate"]))
            .unwrap_err()
            .contains("not an option"));
    }
}
