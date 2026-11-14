//! The DAC board's two choices: which input it takes, and which filter it
//! reconstructs with.
//!
//! Both are controls of the board's ALSA mixer, each a list of names with
//! one of them chosen. They are read and set through `amixer`, which every
//! Volumio has, so no sound library is linked into this program. The card
//! is found by its name among the system's sound cards, whatever number it
//! was given.

use std::process::Command;

/// The card's name, as the kernel lists it.
pub const CARD: &str = "I-Sabre Q2M DAC";
/// The mixer's control for the input, and the name of the input that is
/// the player itself.
pub const INPUT: &str = "I2S/SPDIF Select";
pub const PLAYER_INPUT: &str = "I2S";
/// The mixer's control for the filter.
pub const FILTER: &str = "FIR Filter Type";

const CARDS: &str = "/proc/asound/cards";

/// A control of the mixer: the names it can take, and the one it has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub items: Vec<String>,
    pub chosen: String,
}

impl Choice {
    /// The name after the chosen one, the first after the last.
    pub fn next(&self) -> Option<&str> {
        if self.items.is_empty() {
            return None;
        }
        let at = self.items.iter().position(|item| *item == self.chosen);
        let next = at.map_or(0, |i| (i + 1) % self.items.len());
        Some(&self.items[next])
    }
}

/// What runs `amixer`: the system's own, or a stand-in for the tests.
pub trait Mixer {
    /// Run with these arguments and give back what it printed.
    fn run(&self, args: &[&str]) -> Result<String, String>;
}

pub struct Amixer;

impl Mixer for Amixer {
    fn run(&self, args: &[&str]) -> Result<String, String> {
        let out = Command::new("amixer")
            .args(args)
            .output()
            .map_err(|e| format!("amixer: {e}"))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            Err(format!(
                "amixer {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }
}

/// The number of the card named `name` in the kernel's list of sound
/// cards: lines such as ` 0 [DAC            ]: I-SabreQ2M - I-Sabre Q2M DAC`.
pub fn card_in(cards: &str, name: &str) -> Option<u32> {
    cards
        .lines()
        .filter(|line| line.contains(name))
        .find_map(|line| line.split_whitespace().next()?.parse().ok())
}

/// What `amixer sget` says of a control that is a list of names:
///
/// ```text
///   Items: 'brick wall' 'minimum phase slow'
///   Item0: 'brick wall'
/// ```
pub fn parse(said: &str) -> Option<Choice> {
    let quoted = |line: &str| -> Vec<String> {
        line.split('\'')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect()
    };
    let mut items = None;
    let mut chosen = None;
    for line in said.lines() {
        let line = line.trim_start();
        if let Some(rest) = line.strip_prefix("Items:") {
            items = Some(quoted(rest));
        } else if let Some(rest) = line.strip_prefix("Item0:") {
            chosen = quoted(rest).into_iter().next();
        }
    }
    Some(Choice {
        items: items.filter(|i| !i.is_empty())?,
        chosen: chosen?,
    })
}

/// What `amixer scontents` says of every control that is a list of names:
/// each by its name.
pub fn parse_all(said: &str) -> Vec<(String, Choice)> {
    let mut found = Vec::new();
    let mut name: Option<String> = None;
    let mut block = String::new();
    let mut close = |name: &mut Option<String>, block: &mut String| {
        if let (Some(name), Some(choice)) = (name.take(), parse(block)) {
            found.push((name, choice));
        }
        block.clear();
    };
    for line in said.lines() {
        if let Some(rest) = line.strip_prefix("Simple mixer control '") {
            close(&mut name, &mut block);
            name = rest.split('\'').next().map(str::to_string);
        } else {
            block.push_str(line);
            block.push('\n');
        }
    }
    close(&mut name, &mut block);
    found
}

pub struct Dac<M: Mixer> {
    mixer: M,
    card: String,
}

impl Dac<Amixer> {
    /// The DAC board, where the system has one.
    pub fn find() -> Result<Self, String> {
        let cards = std::fs::read_to_string(CARDS).map_err(|e| format!("{CARDS}: {e}"))?;
        let card = card_in(&cards, CARD).ok_or_else(|| format!("no sound card named {CARD}"))?;
        Ok(Self::on(Amixer, card))
    }
}

impl<M: Mixer> Dac<M> {
    pub fn on(mixer: M, card: u32) -> Self {
        Self {
            mixer,
            card: card.to_string(),
        }
    }

    /// What a control is set to, and what else it can be.
    pub fn read(&self, control: &str) -> Result<Choice, String> {
        let said = self.mixer.run(&["-c", &self.card, "sget", control])?;
        parse(&said).ok_or_else(|| format!("{control}: not a list of names: {}", said.trim()))
    }

    /// Every control of the card that is a list of names, in one run.
    pub fn choices(&self) -> Result<Vec<(String, Choice)>, String> {
        Ok(parse_all(&self.mixer.run(&[
            "-c",
            &self.card,
            "scontents",
        ])?))
    }

    /// Set a control to the name after the one it has, and say what the
    /// mixer has it at afterwards.
    pub fn step(&self, control: &str) -> Result<Choice, String> {
        let now = self.read(control)?;
        let Some(next) = now.next() else {
            return Ok(now);
        };
        self.mixer
            .run(&["-c", &self.card, "-q", "sset", control, next])?;
        self.read(control)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::cell::RefCell;

    const CARDS: &str = "\
 0 [vc4hdmi0       ]: vc4-hdmi - vc4-hdmi-0
                      vc4-hdmi-0
 2 [DAC            ]: I-SabreQ2M - I-Sabre Q2M DAC
                      I-Sabre Q2M DAC
";

    const FILTERS: &[&str] = &[
        "brick wall",
        "corrected minimum phase fast",
        "minimum phase slow",
        "minimum phase fast",
        "linear phase slow",
        "linear phase fast",
        "apodizing fast",
    ];

    /// A mixer with the board's two controls, that notes what it is asked.
    pub struct Fake {
        pub filter: RefCell<String>,
        pub input: RefCell<String>,
        pub asked: RefCell<Vec<String>>,
        pub deaf: bool,
    }

    impl Fake {
        pub fn new() -> Self {
            Self {
                filter: RefCell::new("brick wall".into()),
                input: RefCell::new("I2S".into()),
                asked: RefCell::new(Vec::new()),
                deaf: false,
            }
        }

        fn said(control: &str, items: &[&str], chosen: &str) -> String {
            let list: Vec<String> = items.iter().map(|i| format!("'{i}'")).collect();
            format!(
                "Simple mixer control '{control}',0\n  Capabilities: enum\n  Items: {}\n  Item0: '{chosen}'\n",
                list.join(" ")
            )
        }
    }

    impl Mixer for Fake {
        fn run(&self, args: &[&str]) -> Result<String, String> {
            self.asked.borrow_mut().push(args.join(" "));
            if self.deaf {
                return Err("amixer: Mixer attach hw:2 error: No such device".into());
            }
            match args {
                ["-c", "2", "sget", FILTER] => {
                    Ok(Fake::said(FILTER, FILTERS, &self.filter.borrow()))
                }
                ["-c", "2", "sget", INPUT] => {
                    Ok(Fake::said(INPUT, &["I2S", "SPDIF"], &self.input.borrow()))
                }
                ["-c", "2", "scontents"] => Ok(format!(
                    "Simple mixer control 'Digital',0\n  Capabilities: pvolume pswitch\n  Mono: Playback 74 [74%] [on]\n{}{}",
                    Fake::said(FILTER, FILTERS, &self.filter.borrow()),
                    Fake::said(INPUT, &["I2S", "SPDIF"], &self.input.borrow())
                )),
                ["-c", "2", "-q", "sset", FILTER, name] => {
                    *self.filter.borrow_mut() = name.to_string();
                    Ok(String::new())
                }
                ["-c", "2", "-q", "sset", INPUT, name] => {
                    *self.input.borrow_mut() = name.to_string();
                    Ok(String::new())
                }
                other => Err(format!("unexpected: {other:?}")),
            }
        }
    }

    #[test]
    fn the_card_is_found_by_its_name_whatever_its_number() {
        assert_eq!(card_in(CARDS, CARD), Some(2));
        assert_eq!(card_in(CARDS, "vc4-hdmi-0"), Some(0));
        assert_eq!(card_in(CARDS, "HiFiBerry"), None);
        assert_eq!(card_in("", CARD), None);
        assert_eq!(card_in("--- no soundcards ---\n", CARD), None);
    }

    #[test]
    fn a_list_of_names_is_read_with_the_one_chosen() {
        let choice = parse(&Fake::said(FILTER, FILTERS, "minimum phase slow")).unwrap();
        assert_eq!(choice.items.len(), 7);
        assert_eq!(choice.items[1], "corrected minimum phase fast");
        assert_eq!(choice.chosen, "minimum phase slow");
        assert_eq!(choice.next(), Some("minimum phase fast"));
        // What is not such a control is not read as one.
        assert_eq!(
            parse("Simple mixer control 'Digital',0\n  Capabilities: pvolume\n"),
            None
        );
        assert_eq!(parse("  Items:\n  Item0: 'x'\n"), None);
        assert_eq!(parse(""), None);
    }

    #[test]
    fn every_list_of_names_of_the_card_is_read_in_one_run() {
        let dac = Dac::on(Fake::new(), 2);
        *dac.mixer.input.borrow_mut() = "SPDIF".into();
        let all = dac.choices().unwrap();
        let names: Vec<&str> = all.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, [FILTER, INPUT], "the volume is not a list of names");
        assert_eq!(all[0].1.chosen, "brick wall");
        assert_eq!(all[1].1.chosen, "SPDIF");
        assert_eq!(dac.mixer.asked.borrow().len(), 1);
        assert_eq!(parse_all(""), vec![]);
    }

    #[test]
    fn the_name_after_the_last_is_the_first() {
        let last = Choice {
            items: vec!["I2S".into(), "SPDIF".into()],
            chosen: "SPDIF".into(),
        };
        assert_eq!(last.next(), Some("I2S"));
        let unknown = Choice {
            chosen: "elsewhere".into(),
            ..last.clone()
        };
        assert_eq!(unknown.next(), Some("I2S"));
        let empty = Choice {
            items: vec![],
            chosen: String::new(),
        };
        assert_eq!(empty.next(), None);
    }

    #[test]
    fn a_step_sets_the_next_name_and_says_what_the_mixer_then_has() {
        let dac = Dac::on(Fake::new(), 2);
        assert_eq!(dac.read(FILTER).unwrap().chosen, "brick wall");
        let after = dac.step(FILTER).unwrap();
        assert_eq!(after.chosen, "corrected minimum phase fast");
        assert_eq!(
            dac.mixer.asked.borrow().last().unwrap(),
            "-c 2 sget FIR Filter Type",
            "read back after it was set"
        );
        assert!(dac
            .mixer
            .asked
            .borrow()
            .iter()
            .any(|a| a == "-c 2 -q sset FIR Filter Type corrected minimum phase fast"));
        assert_eq!(dac.step(INPUT).unwrap().chosen, "SPDIF");
        assert_eq!(dac.step(INPUT).unwrap().chosen, PLAYER_INPUT);
    }

    #[test]
    fn a_mixer_that_does_not_answer_is_an_error_with_its_words() {
        let dac = Dac::on(
            Fake {
                deaf: true,
                ..Fake::new()
            },
            2,
        );
        assert!(dac.read(INPUT).unwrap_err().contains("No such device"));
        assert!(dac.step(FILTER).unwrap_err().contains("No such device"));
    }
}
