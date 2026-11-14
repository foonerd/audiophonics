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
//! within its start limit. A receiver it cannot find is reported once and
//! the panel carries on without the remote.

mod config;
mod farewell;
mod frame;
mod greeting;
mod icons;
mod keys;
mod layout;
mod orbit;
mod render;
mod scroll;
mod segments;
mod ssd1322;
mod status;
mod text;
mod unifont;

use audiophonics_base::clock::Clock;
use audiophonics_base::idle::{Idle, Level};
use audiophonics_base::{board, debug, error, info, log, net};
use audiophonics_player::probe::{Probe, Report};
use audiophonics_player::{Player, Volumio};
use audiophonics_remote::{rc, reader, Event, Press};
use config::Config;
use frame::Frame;
use keys::Action;
use status::Status;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use unifont::Unifont;

const NAME: &str = "evosabre-panel";

const USAGE: &str = "\
usage: evosabre-panel [--config FILE]
       evosabre-panel --render DIR [--assets DIR]
       evosabre-panel --version

  --config FILE  the plugin's settings, as Volumio's configuration store
                 keeps them; without it, the installed plugin's own
  --render DIR   draw every screen into DIR as PGM images, and leave
  --assets DIR   with --render: the logo and the Unifont file to draw with
  --version      say which build this is";

/// Redraw rate while a line scrolls: one pixel per frame at the marquee's
/// speed.
const SCROLL_FRAME: Duration = Duration::from_millis(1000 / scroll::SPEED as u64);
/// How often the address is looked up again, in polls.
const ADDRESS_EVERY: u32 = 10;

/// What the loop wakes for besides time.
enum Msg {
    Remote(Event),
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Run { config: String },
    Render { dir: String, assets: Option<String> },
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

/// Find the receiver, see its decoder on, and start reading it. Returns
/// whether it is being read, and what became of it in a sentence.
fn start_remote(tx: &Sender<Msg>) -> (bool, String) {
    let receiver = match rc::find() {
        Ok(r) => r,
        Err(e) => {
            error!("remote: {e}");
            return (false, format!("not found: {e}"));
        }
    };
    let decoder = match rc::ensure_decoder(&receiver) {
        rc::Decoder::On => {
            info!("remote: {} decodes nec", receiver.name);
            None
        }
        rc::Decoder::SwitchedOn => {
            info!("remote: nec switched on for {}", receiver.name);
            None
        }
        rc::Decoder::Off { enabled, why } => {
            error!(
                "remote: {} does not decode nec (on: {enabled}) and it could not be switched on: {why}",
                receiver.name
            );
            Some(format!(
                ", but its nec decoder is off and could not be switched on: {why}"
            ))
        }
    };
    // Read it even with the decoder off: it may be switched on from
    // outside, and the buttons then arrive without a restart.
    match reader::spawn(receiver.event.clone(), tx.clone(), Msg::Remote) {
        Ok(()) => {
            info!(
                "remote: reading {} at {}",
                receiver.name,
                receiver.event.display()
            );
            (
                true,
                format!(
                    "reading {} at {}{}",
                    receiver.name,
                    receiver.event.display(),
                    decoder.unwrap_or_default()
                ),
            )
        }
        Err(e) => {
            error!("remote: {e}");
            (
                false,
                format!("{} found, but it cannot be read: {e}", receiver.name),
            )
        }
    }
}

const REMOTE_OFF: &str = "switched off on the settings page";

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
    let (mut remote_read, mut remote_line) = if cfg.remote {
        start_remote(&tx)
    } else {
        (false, REMOTE_OFF.to_string())
    };
    status.receiver(remote_line.clone());

    let mut volumio = Volumio::new(cfg.player_base.clone(), cfg.request_timeout);
    let mut probe = Probe::default();
    let mut screen = layout::Playing::default();
    let mut selector = layout::Selector::default();
    let mut font = Unifont::new(&cfg.unifont_path);
    let (clock, warning) = Clock::new();
    if let Some(w) = warning {
        error!("{w}");
    }
    let mut ip = net::address().unwrap_or_else(|| "no network".into());
    let mut polls: u32 = 0;
    let mut idle = Idle::new(greeted_at, cfg.dim_after, cfg.blank_after);
    let mut level = Level::Full;
    let mut greeting_up = true;
    let mut moving = false;
    let mut next_poll = Instant::now();

    loop {
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
            if new.remote && !remote_read {
                (remote_read, remote_line) = start_remote(&tx);
            }
            status.receiver(if new.remote {
                remote_line.clone()
            } else {
                REMOTE_OFF.to_string()
            });
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
        if now >= next_poll {
            match probe.observe(volumio.state()) {
                Some(Report::Changed(line)) => info!("{line}"),
                Some(Report::Outage(line)) => error!("{line}"),
                None => {}
            }
            idle.player(now, probe.state().is_some_and(|s| s.is_playing()));
            polls += 1;
            if polls.is_multiple_of(ADDRESS_EVERY) {
                ip = net::address().unwrap_or_else(|| "no network".into());
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
            match &time {
                Some(t) => layout::clock(&mut frame, &state, &ip, t),
                None => screen.render(&mut frame, &state, &ip, now, &mut font),
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
                            error!("remote: the receiver stopped answering: {why}");
                            remote_read = false;
                            remote_line = format!("lost: {why}");
                            status.receiver(remote_line.clone());
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
