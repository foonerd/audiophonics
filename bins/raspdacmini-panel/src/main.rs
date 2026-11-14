//! raspdacmini-panel: the RaspDac Mini's display.
//!
//! This build draws the screens: `--render` writes every one of them as an
//! image, from the code that will draw the glass.

mod art;
mod canvas;
mod icons;
mod mark;
mod render;
mod screens;
mod text;
mod ticker;

use std::path::Path;
use std::process::ExitCode;

const NAME: &str = "raspdacmini-panel";

/// Where the installed plugin keeps its faces and its mark.
const ASSETS: &str = "/data/plugins/system_hardware/raspdac_mini_lcd/assets";

const USAGE: &str = "\
usage: raspdacmini-panel --render DIR [--assets DIR]
       raspdacmini-panel --version

  --render DIR   draw every screen into DIR as PPM images, and leave
  --assets DIR   with --render: the faces and the mark to draw with;
                 without it, the installed plugin's own
  --version      say which build this is";

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Render { dir: String, assets: String },
    Version,
    Help,
}

fn parse(args: &[String]) -> Result<Command, String> {
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
            "--render" => render = Some(value("a directory")?),
            "--assets" => assets = Some(value("a directory")?),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    match (render, assets) {
        (Some(dir), assets) => Ok(Command::Render {
            dir,
            assets: assets.unwrap_or_else(|| ASSETS.to_string()),
        }),
        (None, Some(_)) => Err("--assets goes with --render".to_string()),
        (None, None) => Ok(Command::Help),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args) {
        Ok(Command::Version) => {
            println!("{NAME} {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Command::Help) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Command::Render { dir, assets }) => {
            match render::all(Path::new(&dir), Path::new(&assets)) {
                Ok(files) => {
                    for file in files {
                        println!("{}", file.display());
                    }
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{NAME}: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Err(e) => {
            eprintln!("{NAME}: {e}\n{USAGE}");
            ExitCode::from(2)
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
    fn the_arguments_are_read() {
        assert_eq!(parse(&args(&["--version"])), Ok(Command::Version));
        assert_eq!(parse(&args(&["--help"])), Ok(Command::Help));
        assert_eq!(parse(&args(&[])), Ok(Command::Help));
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
                assets: ASSETS.into()
            })
        );
        assert!(parse(&args(&["--render"])).unwrap_err().contains("needs"));
        assert!(parse(&args(&["--assets", "a"]))
            .unwrap_err()
            .contains("--render"));
        assert!(parse(&args(&["--frobnicate"]))
            .unwrap_err()
            .contains("unknown"));
    }
}
