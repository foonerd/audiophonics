//! raspdacmini-panel: the RaspDac Mini's display and its remote.
//!
//! Start: open the display and show the mark, find the infrared receiver
//! and the DAC board. Then, for as long as the process lives: poll the
//! player at the panel's interval, take a button the moment it arrives,
//! fetch the cover of what plays away from this loop, and send the glass
//! the part of the picture that changed. Every change of player state,
//! every button and every outage goes to the journal once, so the unit can
//! be checked from a terminal as well as from the glass.
//!
//! The panel is driven one of two ways, as the settings say: see `panel`.
//! Where it is a framebuffer, the system's boot splash draws on it too,
//! at the start of a boot, while the system goes down and through a system
//! update; for as long as the splash runs, this program draws nothing.
//!
//! While nothing plays and nothing is touched the screen goes black after
//! `sleep_after`, and comes back with a button, with playback, or with a
//! change of volume made elsewhere. On SIGTERM the screen is made black
//! before the process leaves; unless the player is going down, which the
//! plugin announces by signal, in which case the panel says so first: see
//! `audiophonics_base::leaving`. SIGHUP reads the settings again.
//!
//! A player that does not answer is not an error: Volumio restarts, and
//! the panel outlives it, showing the mark until it is back. The process
//! exits only for a display it cannot open or a bus write that fails, each
//! said plainly, each retried by systemd within its start limit. A
//! receiver or a DAC board that is not there is reported once and looked
//! for again every few seconds.

mod art;
mod canvas;
mod config;
mod cover;
mod dac;
mod fbdev;
mod icons;
mod ili9341;
mod keys;
mod mark;
mod pages;
mod panel;
mod plymouth;
mod render;
mod screens;
mod text;
mod ticker;

use audiophonics_base::clock::Clock;
use audiophonics_base::idle::{Idle, Level, Playing};
use audiophonics_base::leaving::{self, Leaving};
use audiophonics_base::{board, debug, error, info, log, net};
use audiophonics_player::probe::{Probe, Report};
use audiophonics_player::{Player, Volumio};
use audiophonics_remote::watch::{self, Watch};
use audiophonics_remote::{Event, Press, Status};
use canvas::{Area, Canvas, Frame, Rgb};
use config::Config;
use cover::{Arrived, Covers, Shelf};
use dac::{Amixer, Dac};
use keys::{Action, Keys};
use mark::Mark;
use pages::{Page, Pages};
use panel::Panel;
use screens::{Screens, Shown};
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use text::Faces;

const NAME: &str = "raspdacmini-panel";

/// The name the plugin answers to at the player, for what only the player
/// can do.
const PLUGIN: &str = "raspdac_mini_lcd";

const USAGE: &str = "\
usage: raspdacmini-panel [--config FILE]
       raspdacmini-panel --render DIR [--assets DIR]
       raspdacmini-panel --cover ADDRESS [--player URL]
       raspdacmini-panel --version

  --config FILE    the plugin's settings, as Volumio's configuration store
                   keeps them; without it, the installed plugin's own
  --render DIR     draw every screen into DIR as PPM images, and leave
  --assets DIR     with --render: the faces and the mark to draw with;
                   without it, the installed plugin's own
  --cover ADDRESS  fetch a cover, by the player's name for it, and write
                   the background it makes to standard output: what the
                   panel runs for each cover, so that no cover can stop it
  --player URL     with --cover: where the player is
  --version        say which build this is";

/// A frame of whatever moves: a line that runs, a page that slides.
const FRAME: Duration = Duration::from_millis(40);
/// How often the address is looked up again, in polls.
const ADDRESS_EVERY: u32 = 10;
/// How often the time zone is read again, in polls: it may be changed while
/// the panel runs.
const ZONE_EVERY: u32 = 120;
/// How long after the player last played it still counts as playing: the
/// player reports a stop between two tracks.
const REST_GRACE: Duration = Duration::from_secs(10);
/// How long the mark stays at the least, so a service started beside a
/// player that is already up does not flash it.
const GREETING_HOLD: Duration = Duration::from_millis(1500);
/// How often the DAC board's input and filter are read while the screen is
/// up: they can be changed from elsewhere.
const BOARD_EVERY: Duration = Duration::from_secs(3);
/// How often it is asked whether the system's splash has the screen.
const SPLASH_EVERY: Duration = Duration::from_millis(500);

/// What the loop wakes for besides time.
enum Msg {
    Remote(Event),
    Cover(Arrived),
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Run { config: String },
    Render { dir: String, assets: String },
    Cover { albumart: String, player: String },
    Version,
    Help,
}

fn parse(args: &[String]) -> Result<Command, String> {
    let defaults = Config::default();
    let mut config = config::DEFAULT_PATH.to_string();
    let mut render = None;
    let mut assets = None;
    let mut cover = None;
    let mut player = None;
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
            "--cover" => cover = Some(value("an address")?),
            "--player" => player = Some(value("an address")?),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    match (render, cover) {
        (Some(_), Some(_)) => Err("--render and --cover are two different things".to_string()),
        (Some(dir), None) if player.is_none() => Ok(Command::Render {
            dir,
            assets: assets.unwrap_or(defaults.assets),
        }),
        (None, Some(albumart)) if assets.is_none() => Ok(Command::Cover {
            albumart,
            player: player.unwrap_or(defaults.player_base),
        }),
        (None, None) if assets.is_none() && player.is_none() => Ok(Command::Run { config }),
        _ if assets.is_some() => Err("--assets goes with --render".to_string()),
        _ => Err("--player goes with --cover".to_string()),
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
        Ok(Command::Render { dir, assets }) => render::all(Path::new(&dir), Path::new(&assets))
            .map(|files| {
                for file in files {
                    println!("{}", file.display());
                }
            }),
        Ok(Command::Cover { albumart, player }) => cover::helper(&player, &albumart),
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

/// A flag one or more signals raise.
fn raised_by(signals: &[i32]) -> Result<Arc<AtomicBool>, String> {
    let flag = Arc::new(AtomicBool::new(false));
    for &signal in signals {
        signal_hook::flag::register(signal, Arc::clone(&flag))
            .map_err(|e| format!("signal handler: {e}"))?;
    }
    Ok(flag)
}

/// The DAC board as the loop keeps it: its input and its filter as last
/// read, and when to read them again.
struct Board {
    dac: Option<Dac<Amixer>>,
    input: String,
    filter: String,
    said: String,
    look_at: Instant,
}

impl Board {
    fn new(now: Instant) -> Self {
        Self {
            dac: None,
            input: String::new(),
            filter: String::new(),
            said: String::new(),
            look_at: now,
        }
    }

    /// What there is to say of the board, said when it changes.
    fn say(&mut self, line: String, bad: bool) {
        if line != self.said {
            if bad {
                error!("dac: {line}");
            } else {
                info!("dac: {line}");
            }
            self.said = line;
        }
    }

    /// Find the board where it has not been found, and read its input and
    /// its filter.
    fn look(&mut self, now: Instant) {
        self.look_at = now + BOARD_EVERY;
        if self.dac.is_none() {
            match Dac::find() {
                Ok(dac) => self.dac = Some(dac),
                Err(e) => {
                    self.say(e, true);
                    return;
                }
            }
        }
        let Some(dac) = &self.dac else { return };
        match dac.choices() {
            Ok(choices) => {
                let named = |name: &str| {
                    choices
                        .iter()
                        .find(|(control, _)| control == name)
                        .map(|(_, choice)| choice.chosen.clone())
                        .unwrap_or_default()
                };
                let (input, filter) = (named(dac::INPUT), named(dac::FILTER));
                self.say(format!("input {input}, filter {filter}"), false);
                self.input = input;
                self.filter = filter;
            }
            Err(e) => {
                // The card went, or was never the board: look for it afresh.
                self.dac = None;
                self.say(e, true);
            }
        }
    }

    /// Step one of the board's two choices on, as a button asks.
    fn step(&mut self, control: &str, now: Instant) {
        let Some(dac) = &self.dac else {
            error!("dac: {control} cannot be changed: {}", self.said);
            return;
        };
        match dac.step(control) {
            Ok(choice) => {
                info!("dac: {control} is now {}", choice.chosen);
                if control == dac::INPUT {
                    self.input = choice.chosen;
                } else {
                    self.filter = choice.chosen;
                }
                self.said = format!("input {}, filter {}", self.input, self.filter);
                self.look_at = now + BOARD_EVERY;
            }
            Err(e) => error!("dac: {control}: {e}"),
        }
    }

    /// The input, where it is one other than the player.
    fn other_input(&self) -> Option<&str> {
        (!self.input.is_empty() && self.input != dac::PLAYER_INPUT).then_some(self.input.as_str())
    }
}

/// Send the glass the part of a canvas that is not on it yet; all of it
/// where what is on the glass is not known.
fn present(panel: &mut Panel, shown: &mut Option<Frame>, canvas: &Canvas) -> Result<(), String> {
    let frame = Frame::from(canvas);
    let area = match shown {
        Some(was) => frame.changed(was),
        None => Some(Area::ALL),
    };
    if let Some(area) = area {
        panel.show(&frame, area)?;
        *shown = Some(frame);
    }
    Ok(())
}

/// The mark with a word under it, or a black screen where the settings
/// have the mark switched off.
fn splash(cfg: &Config, mark: Option<&Mark>, word: &str, faces: &mut Faces) -> Canvas {
    let mut canvas = Canvas::new();
    if cfg.splash {
        screens::splash(&mut canvas, mark, word, faces);
    }
    canvas
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
    let mut going: Option<Leaving> = None;

    let display = |e: String| format!("display: {e}");
    let (mut panel, found) = Panel::open(&cfg).map_err(display)?;
    info!("display: {found}");
    // A panel this program has to itself was cleared as it was opened. One
    // it shares shows whatever was drawn on it last, and for as long as
    // the system's splash runs it is the splash's.
    let mut shown = if panel.shared() {
        None
    } else {
        Some(Frame::black())
    };
    let mut yielded = panel.shared() && plymouth::running();
    let mut splash_asked = Instant::now();
    if yielded {
        info!("display: the system's splash has the screen");
    }

    let assets = Path::new(&cfg.assets).to_path_buf();
    let mut faces = Faces::open(&assets);
    let mark = match Mark::open(&assets) {
        Ok(mark) => Some(mark),
        Err(e) => {
            error!("mark: {e}; the words are shown without it");
            None
        }
    };
    let greeting = splash(&cfg, mark.as_ref(), &cfg.starting_text, &mut faces);
    if !yielded {
        present(&mut panel, &mut shown, &greeting).map_err(display)?;
    }
    let started = Instant::now();

    let (tx, rx) = mpsc::channel::<Msg>();
    let mut status = Status::new();
    let mut remote = Watch::new(started);
    if !cfg.remote {
        status.receiver(watch::OFF);
    }
    let mut keys = Keys::default();

    let mut volumio = Volumio::new(cfg.player_base.clone(), cfg.request_timeout);
    let mut probe = Probe::default();
    let covers = {
        let tx = tx.clone();
        Covers::start(cfg.player_base.clone(), move |arrived| {
            let _ = tx.send(Msg::Cover(arrived));
        })?
    };
    let mut shelf = Shelf::new();
    let no_cover = art::none();
    let mut dac_board = Board::new(started);
    let mut screens = Screens::new(started);
    let mut pages = Pages::default();
    let mut queue: Option<u64> = None;
    let mut queue_of: Option<(Option<u64>, String)> = None;

    let (mut clock, warning) = Clock::new();
    if let Some(w) = warning {
        error!("{w}");
    }
    let mut ip = net::address().unwrap_or_else(|| "no network".into());
    let mut polls: u32 = 0;
    // The screen is never dimmed, only made black, and only while nothing
    // plays.
    let mut idle = Idle::new(started, Duration::ZERO, cfg.sleep_after);
    let mut steady = Playing::new(REST_GRACE);
    let mut asleep = false;
    let mut touched: Option<(String, String, Option<u8>)> = None;
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
            idle.retime(Duration::ZERO, new.sleep_after);
            volumio = Volumio::new(new.player_base.clone(), new.request_timeout);
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

        // A panel that is a framebuffer is the system's splash's while that
        // runs: nothing is drawn, and what it leaves on the glass is not
        // what was drawn here last.
        if panel.shared() && splash_asked.elapsed() >= SPLASH_EVERY {
            splash_asked = Instant::now();
            let has_it = plymouth::running();
            if has_it != yielded {
                yielded = has_it;
                shown = None;
                asleep = false;
                if yielded {
                    info!("display: the system's splash has the screen");
                } else {
                    info!("display: the screen is this program's again");
                }
            }
        }

        // The player is going down: say so, in place of whatever was up.
        let announced = if going_off.swap(false, Ordering::Relaxed) {
            Some(Leaving::Off)
        } else if going_restart.swap(false, Ordering::Relaxed) {
            Some(Leaving::Restart)
        } else {
            None
        };
        if let Some(way) = announced {
            if going.is_none() {
                let word = match way {
                    Leaving::Off => {
                        info!("leaving: power off");
                        &cfg.farewell_text
                    }
                    Leaving::Restart => {
                        info!("leaving: restart");
                        &cfg.restart_text
                    }
                };
                if !yielded {
                    let canvas = splash(&cfg, mark.as_ref(), word, &mut faces);
                    present(&mut panel, &mut shown, &canvas).map_err(display)?;
                }
                going = Some(way);
            }
        }
        if stop.load(Ordering::Relaxed) {
            let black = Canvas::new();
            let way = leaving::at_stop(going);
            if way == leaving::AtStop::HoldThenBlank {
                std::thread::sleep(leaving::HOLD);
            }
            // The system's splash, where it has come up to see the system
            // down, keeps the screen.
            let splash_has_it = panel.shared() && plymouth::running();
            match way {
                _ if splash_has_it => {
                    info!("stopping: the screen left to the system's splash");
                }
                leaving::AtStop::Keep => {
                    info!("stopping: the word left on the glass for the mark to replace");
                }
                leaving::AtStop::Blank | leaving::AtStop::HoldThenBlank => {
                    present(&mut panel, &mut shown, &black).map_err(display)?;
                    info!("stopping: screen made black");
                }
            }
            return Ok(());
        }
        if going.is_some() {
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
            // A change made from another surface is a touch too.
            let seen = probe
                .state()
                .map(|s| (s.status.clone(), s.title.clone(), s.volume));
            if seen != touched {
                if touched.is_some() {
                    idle.activity(now);
                }
                touched = seen;
            }
            polls += 1;
            if polls.is_multiple_of(ADDRESS_EVERY) {
                ip = net::address().unwrap_or_else(|| "no network".into());
            }
            if polls.is_multiple_of(ZONE_EVERY) {
                (clock, _) = Clock::new();
            }
            next_poll = now + cfg.poll_interval;
        }
        if let Some(action) = keys.due(now) {
            if act(action, &volumio, &mut dac_board, &mut pages, now) {
                next_poll = now;
            }
        }

        if yielded {
            // Not this program's to draw on for now.
        } else if idle.level(now) == Level::Blank {
            if !asleep {
                present(&mut panel, &mut shown, &Canvas::new()).map_err(display)?;
                info!("screen black");
                asleep = true;
            }
        } else {
            if asleep {
                info!("screen back");
                asleep = false;
                // Whatever changed while it was black is read at once.
                dac_board.look_at = now;
            }
            if now >= dac_board.look_at {
                dac_board.look(now);
            }
            // The cover of what plays: asked for when the track names a new
            // one, and again when one that failed is due.
            let albumart = probe.state().map_or("", |s| s.albumart.as_str());
            if let Some(name) = shelf.playing(albumart, now) {
                covers.want(&name);
            }

            let canvas = match probe.state() {
                // No player yet, or no player any more: the mark.
                None => {
                    pages.reset();
                    splash(&cfg, mark.as_ref(), &cfg.starting_text, &mut faces)
                }
                Some(_) if started.elapsed() < GREETING_HOLD => greeting.clone(),
                Some(state) => {
                    let mut state = state.clone();
                    if state.status.is_empty() {
                        // The player answered without a status, as it does
                        // for a moment while it changes track: that is
                        // stopped, not gone.
                        state.status = "stop".into();
                    }
                    let time = clock.now().map(|t| t.hhmm()).unwrap_or_default();
                    // The queue's length is asked for while the second
                    // page is up, once for each track.
                    if pages.on() == Page::Second {
                        let track = (state.position, state.title.clone());
                        if queue_of.as_ref() != Some(&track) {
                            queue = volumio.queue_length().ok();
                            queue_of = Some(track);
                        }
                    }
                    let what = Shown {
                        state: &state,
                        address: &ip,
                        clock: &time,
                        filter: &dac_board.filter,
                        queue,
                    };
                    let backdrop: &[Rgb] = shelf.backdrop().unwrap_or(&no_cover);
                    let mut canvas = Canvas::new();
                    if let Some(name) = dac_board.other_input() {
                        pages.reset();
                        screens::input(&mut canvas, name, &what, &mut faces);
                    } else {
                        match pages.offset(now) {
                            0 => screens.playing(&mut canvas, backdrop, &what, &mut faces, now),
                            pages::TRAVEL => {
                                screens.details(&mut canvas, backdrop, &what, &mut faces, now)
                            }
                            moved => {
                                let mut second = Canvas::new();
                                screens.playing(&mut canvas, backdrop, &what, &mut faces, now);
                                screens.details(&mut second, backdrop, &what, &mut faces, now);
                                canvas = canvas.slid(&second, moved, screens::DETAILS_BAND.h);
                            }
                        }
                    }
                    canvas
                }
            };
            if let Some(problem) = faces.take_problem() {
                error!("text: {problem}");
            }
            present(&mut panel, &mut shown, &canvas).map_err(display)?;
        }

        // Sleep until the next poll, the next frame of what moves, a button
        // that is still down, a cover to try again or the board to read,
        // whichever is first; or until a button or a cover arrives.
        let now = Instant::now();
        let mut wake = next_poll;
        if let Some(at) = keys.deadline() {
            wake = wake.min(at);
        }
        if !asleep && !yielded {
            let player_up = probe.state().is_some();
            if player_up && (pages.sliding(now) || screens.moving()) {
                wake = wake.min(now + FRAME);
            }
            wake = wake.min(dac_board.look_at);
            if let Some(at) = shelf.retry_at() {
                wake = wake.min(at);
            }
        }
        let wait = wake.saturating_duration_since(now);
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
                            // bring a black screen back; another handset's
                            // are not.
                            if press.key.is_some() {
                                idle.activity(now);
                            }
                            said(&press, &mut status);
                            if let Some(action) = keys.press(&press, now) {
                                if act(action, &volumio, &mut dac_board, &mut pages, now) {
                                    next_poll = now;
                                }
                            }
                        }
                        Msg::Remote(Event::Lost(why)) => {
                            remote.lost(&why, now);
                            status.receiver(remote.line());
                        }
                        Msg::Cover(arrived) => {
                            let (_, problem) = shelf.arrived(arrived, now);
                            if let Some(problem) = problem {
                                error!("{problem}");
                            }
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

/// A frame from the handset, said in the journal and noted for the
/// settings page.
fn said(press: &Press, status: &mut Status) {
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
}

/// Do what a button asked. Returns whether the player was asked for
/// something, so that its state is read again at once.
fn act(
    action: Action,
    player: &Volumio,
    dac_board: &mut Board,
    pages: &mut Pages,
    now: Instant,
) -> bool {
    let asked = match action {
        Action::Transport(cmd) => player.transport(cmd),
        Action::Seek(step) => player.seek(step),
        Action::Ask(what) => player.ask(what),
        // Only the player can shut the player down: its plugin asks it.
        Action::PowerOff => player.plugin(PLUGIN, "shutdown"),
        Action::Input => {
            dac_board.step(dac::INPUT, now);
            return false;
        }
        Action::Filter => {
            dac_board.step(dac::FILTER, now);
            return false;
        }
        Action::Page => {
            // There is no second page to the screen that names an input.
            if dac_board.other_input().is_none() {
                pages.turn(now);
                debug!("remote: page turned");
            }
            return false;
        }
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
                config: config::DEFAULT_PATH.into()
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
    fn the_other_things_it_can_be_asked_are_told_apart() {
        assert_eq!(parse(&args(&["--version"])), Ok(Command::Version));
        assert_eq!(parse(&args(&["--help"])), Ok(Command::Help));
        assert_eq!(
            parse(&args(&["--render", "out", "--assets", "a"])),
            Ok(Command::Render {
                dir: "out".into(),
                assets: "a".into()
            })
        );
        assert_eq!(
            parse(&args(&["--render", "out"])),
            Ok(Command::Render {
                dir: "out".into(),
                assets: Config::default().assets
            })
        );
        assert_eq!(
            parse(&args(&[
                "--cover",
                "/albumart?x=1",
                "--player",
                "http://p:3000"
            ])),
            Ok(Command::Cover {
                albumart: "/albumart?x=1".into(),
                player: "http://p:3000".into()
            })
        );
        assert_eq!(
            parse(&args(&["--cover", "/albumart"])),
            Ok(Command::Cover {
                albumart: "/albumart".into(),
                player: "http://127.0.0.1:3000".into()
            })
        );
    }

    #[test]
    fn arguments_that_do_not_go_together_are_refused_with_a_reason() {
        assert!(parse(&args(&["--render"])).unwrap_err().contains("needs"));
        assert!(parse(&args(&["--assets", "a"]))
            .unwrap_err()
            .contains("--render"));
        assert!(parse(&args(&["--player", "http://p"]))
            .unwrap_err()
            .contains("--cover"));
        assert!(parse(&args(&["--render", "d", "--cover", "/a"]))
            .unwrap_err()
            .contains("two different"));
        assert!(parse(&args(&["--cover", "/a", "--assets", "d"]))
            .unwrap_err()
            .contains("--render"));
        assert!(parse(&args(&["--frobnicate"]))
            .unwrap_err()
            .contains("unknown"));
    }

    #[test]
    fn a_board_that_has_not_been_read_names_no_other_input() {
        let now = Instant::now();
        let mut b = Board::new(now);
        assert_eq!(b.other_input(), None);
        b.input = dac::PLAYER_INPUT.into();
        assert_eq!(b.other_input(), None);
        b.input = "SPDIF".into();
        assert_eq!(b.other_input(), Some("SPDIF"));
        // With no board found, a button that asks for it changes nothing.
        b.step(dac::INPUT, now);
        assert_eq!(b.input, "SPDIF");
    }
}
