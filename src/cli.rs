//! Command line options.
//!
//! Small on purpose: the game needs none of these to run, and every one of
//! them exists so that a screenshot can be taken the same way twice.

use anyhow::{Context, Result};
use isogrid::iso::TilePos;

use crate::park::{Facility, StaffKind, Terrain};
use crate::tool::Tool;

/// How the game was asked to start.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// The seed the park is generated from.
    pub seed: u64,
    /// Where to write a PNG of the frame, if the run is for a screenshot.
    pub screenshot: Option<String>,
    /// How many ticks to simulate before taking it.
    pub ticks: u64,
    /// The zoom to take it at.
    pub zoom: Option<f32>,
    /// The tile to look at.
    pub focus: Option<TilePos>,
    /// The tile to pretend the pointer is over.
    pub hover: Option<TilePos>,
    /// The tool to hold while taking it.
    pub tool: Tool,
    /// Whether to lay the demo coaster before the window opens.
    pub coaster: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            seed: 1,
            screenshot: None,
            ticks: 0,
            zoom: None,
            focus: None,
            hover: None,
            tool: Tool::default(),
            coaster: false,
        }
    }
}

/// What to print when asked for help, or when given something unreadable.
pub const USAGE: &str = "\
openpark — a park simulator

    --seed N            generate the park from this seed (default 1)
    --screenshot PATH   run until --ticks, write a PNG there, and exit
    --ticks N           how many ticks to simulate first (default 0)
    --zoom F            zoom to take the screenshot at
    --focus X,Y         tile to centre the view on
    --hover X,Y         tile to pretend the pointer is over
    --coaster           lay the demo coaster before the window opens
    --tool NAME         inspect, stall, bench, demolish, price-up,
                        price-down, handyman, entertainer, fire, raise,
                        dig, path, grass, dirt or water
    -h, --help          print this
";

impl Options {
    /// Reads options from the arguments, which must not include the program
    /// name.
    ///
    /// # Errors
    ///
    /// Fails on an unknown flag, a missing value, or a value that will not
    /// parse — all of which are worth stopping for rather than guessing at.
    ///
    /// ```
    /// # use openpark::cli::Options;
    /// let options = Options::parse(["--seed", "7", "--ticks", "600"])?;
    /// assert_eq!(options.seed, 7);
    /// assert_eq!(options.ticks, 600);
    /// assert!(options.screenshot.is_none());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn parse<I, S>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut options = Self::default();
        let mut args = args.into_iter();

        while let Some(argument) = args.next() {
            let argument = argument.as_ref().to_owned();
            let mut value = || -> Result<String> {
                args.next()
                    .map(|value| value.as_ref().to_owned())
                    .with_context(|| format!("{argument} needs a value"))
            };

            match value_of(&argument) {
                Flag::Seed => options.seed = parse(&value()?, "--seed")?,
                Flag::Screenshot => options.screenshot = Some(value()?),
                Flag::Ticks => options.ticks = parse(&value()?, "--ticks")?,
                Flag::Zoom => options.zoom = Some(parse(&value()?, "--zoom")?),
                Flag::Focus => options.focus = Some(tile(&value()?, "--focus")?),
                Flag::Hover => options.hover = Some(tile(&value()?, "--hover")?),
                Flag::Tool => options.tool = tool(&value()?)?,
                Flag::Coaster => options.coaster = true,
                Flag::Help => anyhow::bail!("{USAGE}"),
                Flag::Unknown => anyhow::bail!("unknown option {argument}\n\n{USAGE}"),
            }
        }

        Ok(options)
    }

    /// Whether this run is only here to take a picture.
    pub fn is_a_screenshot(&self) -> bool {
        self.screenshot.is_some()
    }
}

/// The flags the game knows.
enum Flag {
    Seed,
    Screenshot,
    Ticks,
    Zoom,
    Focus,
    Hover,
    Tool,
    Coaster,
    Help,
    Unknown,
}

fn value_of(argument: &str) -> Flag {
    match argument {
        "--seed" => Flag::Seed,
        "--screenshot" => Flag::Screenshot,
        "--ticks" => Flag::Ticks,
        "--zoom" => Flag::Zoom,
        "--focus" => Flag::Focus,
        "--hover" => Flag::Hover,
        "--tool" => Flag::Tool,
        "--coaster" => Flag::Coaster,
        "-h" | "--help" => Flag::Help,
        _ => Flag::Unknown,
    }
}

/// Parses one value, saying which flag it belonged to when it will not parse.
fn parse<T: std::str::FromStr>(value: &str, flag: &str) -> Result<T> {
    value
        .parse()
        .map_err(|_| anyhow::anyhow!("{flag} does not understand {value:?}"))
}

/// Parses an `X,Y` pair into a tile.
fn tile(value: &str, flag: &str) -> Result<TilePos> {
    let (x, y) = value
        .split_once(',')
        .with_context(|| format!("{flag} wants X,Y, not {value:?}"))?;
    Ok(TilePos::new(parse(x, flag)?, parse(y, flag)?))
}

/// Parses a tool by name.
fn tool(value: &str) -> Result<Tool> {
    match value {
        "inspect" => Ok(Tool::Inspect),
        "stall" => Ok(Tool::Build(Facility::FoodStall)),
        "bench" => Ok(Tool::Build(Facility::Bench)),
        "demolish" => Ok(Tool::Demolish),
        "price-up" => Ok(Tool::RaisePrice),
        "price-down" => Ok(Tool::LowerPrice),
        "raise" => Ok(Tool::Raise),
        "dig" => Ok(Tool::Lower),
        "path" => Ok(Tool::Lay(Terrain::Path)),
        "grass" => Ok(Tool::Lay(Terrain::Grass)),
        "dirt" => Ok(Tool::Lay(Terrain::Dirt)),
        "water" => Ok(Tool::Lay(Terrain::Water)),
        "handyman" => Ok(Tool::Hire(StaffKind::Handyman)),
        "entertainer" => Ok(Tool::Hire(StaffKind::Entertainer)),
        "fire" => Ok(Tool::Fire),
        other => anyhow::bail!("--tool does not know {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_arguments_means_play_the_game() {
        let options = Options::parse(Vec::<String>::new()).unwrap();
        assert_eq!(options, Options::default());
        assert!(!options.is_a_screenshot());
    }

    #[test]
    fn a_screenshot_run_says_where_and_when() {
        let options = Options::parse([
            "--screenshot",
            "docs/park.png",
            "--ticks",
            "1200",
            "--zoom",
            "1.5",
            "--focus",
            "16,20",
            "--hover",
            "17,21",
            "--tool",
            "stall",
            "--seed",
            "9",
        ])
        .unwrap();

        assert!(options.is_a_screenshot());
        assert_eq!(options.screenshot.as_deref(), Some("docs/park.png"));
        assert_eq!(options.ticks, 1_200);
        assert_eq!(options.zoom, Some(1.5));
        assert_eq!(options.focus, Some(TilePos::new(16, 20)));
        assert_eq!(options.hover, Some(TilePos::new(17, 21)));
        assert_eq!(options.tool, Tool::Build(Facility::FoodStall));
        assert_eq!(options.seed, 9);
    }

    #[test]
    fn every_tool_can_be_named() {
        for (name, expected) in [
            ("inspect", Tool::Inspect),
            ("stall", Tool::Build(Facility::FoodStall)),
            ("bench", Tool::Build(Facility::Bench)),
            ("demolish", Tool::Demolish),
        ] {
            let options = Options::parse(["--tool", name]).unwrap();
            assert_eq!(options.tool, expected, "--tool {name}");
        }
    }

    #[test]
    fn nonsense_is_refused_rather_than_guessed_at() {
        for arguments in [
            vec!["--nope"],
            vec!["--seed"],
            vec!["--seed", "banana"],
            vec!["--ticks", "-4"],
            vec!["--zoom", "wide"],
            vec!["--focus", "16"],
            vec!["--focus", "16,x"],
            vec!["--tool", "bulldozer"],
        ] {
            assert!(
                Options::parse(arguments.clone()).is_err(),
                "{arguments:?} was accepted"
            );
        }
    }

    #[test]
    fn asking_for_help_is_not_a_silent_success() {
        let refused = Options::parse(["--help"]).unwrap_err().to_string();
        assert!(refused.contains("--screenshot"), "{refused}");
    }
}
