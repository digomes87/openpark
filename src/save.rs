//! Saving a park to a file, and getting it back.
//!
//! Every type in [`crate::park`] already derives `serde`, which is most of the
//! work — a park *is* its save file. What this module adds is the two things
//! that make a save file worth trusting: a version stamped on the front, so a
//! park written by a future openpark is refused rather than half-read, and a
//! write that either replaces the old save completely or leaves it alone.
//!
//! The format is JSON. It is not the smallest way to write a park down, but it
//! is readable, diffable, and debuggable with the tools everybody already has,
//! and a park is a few hundred kilobytes rather than a few hundred megabytes.

use serde::{Deserialize, Serialize};

use crate::park::Park;

/// What this openpark writes, and the highest it can read.
///
/// Bumped whenever the shape of [`Park`] changes in a way an older build could
/// not make sense of. Until there is a released version to be compatible with,
/// a save from a different number is simply refused.
pub const FORMAT: u32 = 1;

/// What the default save is called.
pub const DEFAULT_PATH: &str = "openpark.save.json";

/// A park, with the version of the game that wrote it.
///
/// The version comes first in the struct so that it comes first in the file:
/// somebody looking at a save in a text editor should not have to scroll past a
/// grid of terrain to find out what wrote it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveFile {
    /// The format the park below is written in.
    pub version: u32,
    /// The park.
    pub park: Park,
}

impl SaveFile {
    /// Wraps a park up in the current format.
    pub fn of(park: &Park) -> Self {
        Self {
            version: FORMAT,
            park: park.clone(),
        }
    }
}

/// Writes a park out as a save file, as text.
///
/// # Errors
///
/// Fails only if a park cannot be serialised, which would be a bug in the park
/// rather than in the save.
pub fn to_json(park: &Park) -> anyhow::Result<String> {
    let save = SaveFile::of(park);
    serde_json::to_string_pretty(&save).map_err(Into::into)
}

/// Reads a park back from the text of a save file.
///
/// # Errors
///
/// Fails if the text is not a save file at all, or is one written in a format
/// this build does not know.
///
/// ```
/// # use openpark::park::Park;
/// # use openpark::save;
/// let park = Park::new("Forest Frontiers", 32, 32, 1)?;
/// let written = save::to_json(&park)?;
///
/// assert_eq!(save::from_json(&written)?, park);
/// assert!(save::from_json("not a park at all").is_err());
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn from_json(text: &str) -> anyhow::Result<Park> {
    let save: SaveFile = serde_json::from_str(text)
        .map_err(|error| anyhow::anyhow!("this is not an openpark save: {error}"))?;

    anyhow::ensure!(
        save.version == FORMAT,
        "this save is version {} and this openpark reads version {FORMAT}",
        save.version,
    );

    Ok(save.park)
}

#[cfg(not(target_arch = "wasm32"))]
mod files {
    use std::path::{Path, PathBuf};

    use anyhow::{Context, Result};

    use crate::park::Park;

    /// Writes `park` to `path`, replacing whatever was there.
    ///
    /// Written to a temporary file beside the target and renamed over it, so an
    /// interrupted save leaves the previous one intact. A half-written save
    /// that reads as valid JSON is the worst possible outcome, and a rename is
    /// the cheapest way to make it impossible.
    ///
    /// # Errors
    ///
    /// Fails if the park cannot be serialised, or the file cannot be written or
    /// renamed — a directory that does not exist, or one that cannot be written
    /// to.
    pub fn save(park: &Park, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let text = super::to_json(park)?;

        let mut half_way: PathBuf = path.to_path_buf();
        half_way.set_extension("part");

        std::fs::write(&half_way, text)
            .with_context(|| format!("failed to write {}", half_way.display()))?;
        std::fs::rename(&half_way, path)
            .with_context(|| format!("failed to put {} in place", path.display()))?;

        tracing::info!(path = %path.display(), park = park.name(), "saved the park");
        Ok(())
    }

    /// Reads a park back from `path`.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be read, is not a save file, or is one this
    /// build does not know how to read.
    pub fn load(path: impl AsRef<Path>) -> Result<Park> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;

        let park = super::from_json(&text)
            .with_context(|| format!("failed to load {}", path.display()))?;

        tracing::info!(
            path = %path.display(),
            park = park.name(),
            tick = park.tick().get(),
            "loaded a park"
        );
        Ok(park)
    }
}

#[cfg(target_arch = "wasm32")]
mod files {
    use std::path::Path;

    use anyhow::{bail, Result};

    use crate::park::Park;

    /// Browsers have no filesystem to write to.
    ///
    /// # Errors
    ///
    /// Always: there is nowhere to put it. A save in the browser wants local
    /// storage and a different function.
    pub fn save(_park: &Park, path: impl AsRef<Path>) -> Result<()> {
        bail!(
            "saving to {} is not supported in the browser",
            path.as_ref().display()
        )
    }

    /// Browsers have no filesystem to read from.
    ///
    /// # Errors
    ///
    /// Always, for the same reason as [`save`].
    pub fn load(path: impl AsRef<Path>) -> Result<Park> {
        bail!(
            "loading {} is not supported in the browser",
            path.as_ref().display()
        )
    }
}

pub use files::{load, save};

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    use std::path::PathBuf;

    use crate::park::{Facility, StaffKind};
    use isogrid::iso::TilePos;

    /// A park that has had things happen to it, so a save has something to lose.
    fn lived_in_park() -> Park {
        let mut park = Park::new("Forest Frontiers", 32, 32, 7).expect("a valid park");
        park.build(TilePos::new(2, 2), Facility::FoodStall)
            .expect("there should be room");
        park.set_price(TilePos::new(2, 2), 17).expect("it is there");
        park.hire(StaffKind::Handyman).expect("it can afford one");
        park.raise(TilePos::new(5, 5))
            .expect("the land should move");

        for _ in 0..2_000 {
            park.tick_once();
        }
        park
    }

    /// A path in the temporary directory that no other test is using.
    fn somewhere_to_write(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("openpark-{name}-{}.save.json", std::process::id()));
        path
    }

    #[test]
    fn a_park_comes_back_exactly_as_it_went_in() {
        let park = lived_in_park();
        let path = somewhere_to_write("round-trip");

        save(&park, &path).expect("it should save");
        let loaded = load(&path).expect("it should load");
        std::fs::remove_file(&path).ok();

        assert_eq!(loaded, park);
        assert_eq!(loaded.tick(), park.tick());
        assert_eq!(loaded.guests().len(), park.guests().len());
        assert_eq!(loaded.staff().len(), park.staff().len());
    }

    #[test]
    fn a_loaded_park_carries_on_the_same_way() {
        let mut park = lived_in_park();
        let path = somewhere_to_write("carry-on");

        save(&park, &path).expect("it should save");
        let mut loaded = load(&path).expect("it should load");
        std::fs::remove_file(&path).ok();

        // The dice are part of the save, so the next thousand ticks have to
        // play out identically. This is the whole reason the RNG is bundled
        // rather than taken from a crate that reseeds itself.
        for _ in 0..1_000 {
            park.tick_once();
            loaded.tick_once();
        }

        assert_eq!(loaded, park, "the loaded park drifted from the saved one");
    }

    #[test]
    fn the_version_goes_first_in_the_file() {
        let park = Park::new("Forest Frontiers", 8, 8, 1).unwrap();
        let written = to_json(&park).unwrap();

        let first_line = written.lines().nth(1).unwrap_or_default();
        assert!(
            first_line.contains("\"version\""),
            "the version is not at the top of the file: {first_line}"
        );
        assert!(written.contains(&format!("\"version\": {FORMAT}")));
    }

    #[test]
    fn a_save_from_another_version_is_refused_rather_than_half_read() {
        let park = Park::new("Forest Frontiers", 8, 8, 1).unwrap();
        let written = to_json(&park).unwrap();
        let from_the_future = written.replace(
            &format!("\"version\": {FORMAT}"),
            &format!("\"version\": {}", FORMAT + 1),
        );

        let refused = from_json(&from_the_future).expect_err("it should be refused");
        let complaint = refused.to_string();
        assert!(
            complaint.contains(&(FORMAT + 1).to_string()) && complaint.contains("version"),
            "the refusal does not say which version it was: {complaint}"
        );
    }

    #[test]
    fn something_that_is_not_a_save_is_refused() {
        assert!(from_json("").is_err());
        assert!(from_json("{}").is_err(), "an empty object is not a park");
        assert!(from_json("[1, 2, 3]").is_err());
        assert!(
            from_json(r#"{"version": 1, "park": "a park"}"#).is_err(),
            "a park that is a string is not a park"
        );
    }

    #[test]
    fn loading_something_that_is_not_there_says_which_file() {
        let path = somewhere_to_write("missing");
        std::fs::remove_file(&path).ok();

        let refused = load(&path).expect_err("there is no such file");
        assert!(
            format!("{refused:#}").contains("openpark-missing"),
            "the failure does not name the file: {refused:#}"
        );
    }

    #[test]
    fn a_save_replaces_the_one_before_it_and_leaves_no_litter() {
        let path = somewhere_to_write("replace");
        let first = Park::new("The First", 16, 16, 1).unwrap();
        let second = Park::new("The Second", 16, 16, 2).unwrap();

        save(&first, &path).unwrap();
        save(&second, &path).unwrap();

        let loaded = load(&path).unwrap();
        assert_eq!(loaded.name(), "The Second");

        let mut half_way = path.clone();
        half_way.set_extension("part");
        assert!(
            !half_way.exists(),
            "the half-written file was left behind at {}",
            half_way.display()
        );

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_save_that_cannot_be_written_says_so_rather_than_panicking() {
        let park = Park::new("Nowhere", 8, 8, 1).unwrap();
        let path = std::env::temp_dir()
            .join("openpark-no-such-directory")
            .join("deeper")
            .join("park.save.json");

        assert!(save(&park, &path).is_err());
    }
}
