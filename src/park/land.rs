//! The ground: what it is made of, and how high it stands.
//!
//! Two grids of the same size, kept together because nothing sensible can be
//! said about one without the other: a tile of grass eight steps up is a hill,
//! and a tile of water eight steps up is a mistake. Everything that changes the
//! shape of a park goes through here, so the rules about what land is allowed
//! to look like live in one place.
//!
//! Heights are whole steps, not metres. A step is [`isogrid::iso::TileSize`]'s
//! elevation on screen, and the engine does the projecting — this module only
//! ever deals in how many of them a tile is worth.

use core::num::NonZeroU32;

use anyhow::{Context, Result};
use isogrid::grid::Grid;
use isogrid::iso::{GridPoint, TilePos};
use isogrid::rng::Rng;
use serde::{Deserialize, Serialize};

use crate::park::Terrain;

/// A whole turn, for picking where in a wave the hills start.
const TWO_PI: f32 = core::f32::consts::TAU;

/// How many tiles a hill takes to rise and fall, across the map and along it.
///
/// Deliberately not the same number: equal ones make a grid of identical
/// pimples rather than a landscape.
const HILL_WIDTH: f32 = 7.0;
const HILL_DEPTH: f32 = 9.0;

/// How many steps the tallest hill stands.
const HILL_HEIGHT: f32 = 2.5;

/// The land a park is built on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Land {
    terrain: Grid<Terrain>,
    /// How many steps above the base each tile stands. Always the same size as
    /// `terrain`.
    heights: Grid<i16>,
}

impl Land {
    /// The highest a tile can be raised, in steps.
    ///
    /// Sixteen is more than enough for a lift hill and little enough that the
    /// whole park still fits on the screen at the furthest zoom out.
    pub const MAX_HEIGHT: i16 = 16;

    /// The lowest a tile can be dug down to.
    pub const MIN_HEIGHT: i16 = 0;

    /// The biggest height difference a guest will step up or down.
    ///
    /// Anything steeper is a cliff: still land, still buildable on top, but not
    /// something anybody walks off.
    pub const MAX_STEP: i16 = 1;

    /// How much each step of climbing adds to the cost of crossing a tile.
    const CLIMBING: u32 = 4;

    /// Flat land made of `terrain`.
    ///
    /// # Errors
    ///
    /// Fails only if the engine will not allocate a second grid of that size,
    /// which means the first one should not have existed either.
    pub fn flat(terrain: Grid<Terrain>) -> Result<Self> {
        let heights = Grid::filled(terrain.width(), terrain.height(), Self::MIN_HEIGHT)
            .context("the park is too small or too large for the engine to hold")?;

        Ok(Self { terrain, heights })
    }

    /// Land with gentle hills rolled into it.
    ///
    /// Two sine waves crossed, with the phase taken from `rng` so that two
    /// seeds do not produce the same hills. Paths and water are left at the
    /// base: a crossroads that climbed a hill would arrive at the gate as a
    /// staircase, and water that did would be a waterfall.
    ///
    /// # Errors
    ///
    /// As [`Land::flat`].
    pub fn rolling(terrain: Grid<Terrain>, rng: &mut Rng) -> Result<Self> {
        let mut land = Self::flat(terrain)?;

        let (across, along) = (rng.next_f32() * TWO_PI, rng.next_f32() * TWO_PI);
        for tile in land.terrain.positions().collect::<Vec<_>>() {
            if matches!(land.ground(tile), Some(Terrain::Path | Terrain::Water)) {
                continue;
            }

            #[allow(clippy::cast_precision_loss)]
            let (x, y) = (tile.x as f32, tile.y as f32);
            let rise = (x / HILL_WIDTH + across).sin() + (y / HILL_DEPTH + along).sin();

            // Half the map is below zero and stays at the base, so the hills
            // read as hills on a plain rather than as a rolling swell with no
            // flat ground anywhere.
            #[allow(clippy::cast_possible_truncation)]
            let steps = (rise * HILL_HEIGHT) as i16;
            land.heights
                .replace(tile, steps.clamp(Self::MIN_HEIGHT, Self::MAX_HEIGHT));
        }

        land.smooth();
        Ok(land)
    }

    /// Files the generated landscape down until nobody could fall off it.
    ///
    /// Raw sine hills meet the flat paths in a two-step drop, and a two-step
    /// drop is a cliff: it walls the paths in and puts every stall built on the
    /// verge out of reach. So each tile is brought down to one step above its
    /// lowest neighbour, over and over until nothing moves.
    ///
    /// Only ever downwards, so the water and the paths stay where they were put.
    /// Heights are bounded below, so this terminates; the pass count is capped
    /// anyway, because a generator that will not settle should produce a dull
    /// park rather than hang.
    fn smooth(&mut self) {
        for _ in 0..=Self::MAX_HEIGHT {
            let mut filed_anything = false;

            for tile in self.terrain.positions().collect::<Vec<_>>() {
                let Some(height) = self.height_at(tile) else {
                    continue;
                };

                let lowest = tile
                    .neighbours()
                    .iter()
                    .filter_map(|beside| self.height_at(*beside))
                    .min()
                    .unwrap_or(Self::MIN_HEIGHT);

                if height > lowest + Self::MAX_STEP {
                    self.heights.replace(tile, lowest + Self::MAX_STEP);
                    filed_anything = true;
                }
            }

            if !filed_anything {
                return;
            }
        }
    }

    /// What the ground is made of, tile by tile.
    pub const fn terrain(&self) -> &Grid<Terrain> {
        &self.terrain
    }

    /// How high it all stands, tile by tile.
    pub const fn heights(&self) -> &Grid<i16> {
        &self.heights
    }

    /// The width of the land in tiles.
    pub const fn width(&self) -> u32 {
        self.terrain.width()
    }

    /// The height of the land in tiles — its depth on the map, not its
    /// elevation.
    pub const fn height(&self) -> u32 {
        self.terrain.height()
    }

    /// Whether `tile` is part of this land at all.
    pub fn contains(&self, tile: TilePos) -> bool {
        self.terrain.contains(tile)
    }

    /// What `tile` is made of.
    pub fn ground(&self, tile: TilePos) -> Option<Terrain> {
        self.terrain.get(tile).copied()
    }

    /// How many steps up `tile` stands.
    pub fn height_at(&self, tile: TilePos) -> Option<i16> {
        self.heights.get(tile).copied()
    }

    /// How many steps up `tile` stands, counting anything off the map as the
    /// base — for drawing, which would rather have a number than a decision.
    pub fn elevation(&self, tile: TilePos) -> f32 {
        f32::from(self.height_at(tile).unwrap_or(Self::MIN_HEIGHT))
    }

    /// The centre of `tile`, at the height it actually stands.
    pub fn point(&self, tile: TilePos) -> GridPoint {
        let centre = tile.centre();
        GridPoint::new(centre.x, centre.y, self.elevation(tile))
    }

    /// The highest tile in the park, for a camera or a hover that needs to know
    /// how far up to look.
    pub fn highest(&self) -> i16 {
        self.heights
            .as_slice()
            .iter()
            .copied()
            .max()
            .unwrap_or(Self::MIN_HEIGHT)
    }

    /// Replaces what one tile is made of, returning what it was.
    ///
    /// Returns `None` and changes nothing if the tile is outside the land.
    pub fn set_ground(&mut self, tile: TilePos, terrain: Terrain) -> Option<Terrain> {
        self.terrain.replace(tile, terrain)
    }

    /// Raises one tile by a step, returning its new height.
    ///
    /// # Errors
    ///
    /// Fails if the tile is outside the land or already at
    /// [`Land::MAX_HEIGHT`].
    ///
    /// ```
    /// # use openpark::park::{Land, Terrain};
    /// # use isogrid::grid::Grid;
    /// # use isogrid::iso::TilePos;
    /// let mut land = Land::flat(Grid::filled(8, 8, Terrain::Grass)?)?;
    /// assert_eq!(land.raise(TilePos::ORIGIN)?, 1);
    /// assert_eq!(land.lower(TilePos::ORIGIN)?, 0);
    /// assert!(land.lower(TilePos::ORIGIN).is_err(), "the base is the bottom");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn raise(&mut self, tile: TilePos) -> Result<i16> {
        self.reshape(tile, 1)
    }

    /// Digs one tile down by a step, returning its new height.
    ///
    /// # Errors
    ///
    /// Fails if the tile is outside the land or already at
    /// [`Land::MIN_HEIGHT`].
    pub fn lower(&mut self, tile: TilePos) -> Result<i16> {
        self.reshape(tile, -1)
    }

    /// Moves one tile by `steps`, refusing to leave the allowed range.
    fn reshape(&mut self, tile: TilePos, steps: i16) -> Result<i16> {
        let was = self
            .height_at(tile)
            .with_context(|| format!("{tile:?} is outside the park"))?;

        let now = was + steps;
        anyhow::ensure!(
            (Self::MIN_HEIGHT..=Self::MAX_HEIGHT).contains(&now),
            "{tile:?} cannot go past {} steps",
            if steps > 0 {
                Self::MAX_HEIGHT
            } else {
                Self::MIN_HEIGHT
            },
        );

        self.heights.replace(tile, now);
        Ok(now)
    }

    /// Whether somebody standing on `from` could step onto `to`.
    ///
    /// Both tiles have to be walkable ground, and the climb between them no
    /// steeper than [`Land::MAX_STEP`].
    pub fn is_a_step(&self, from: TilePos, to: TilePos) -> bool {
        self.step_cost(from, to).is_some()
    }

    /// What it costs to step from `from` onto `to`, for the pathfinder.
    ///
    /// `None` where nobody can go: off the map, into the water, or up a cliff.
    /// Climbing is dearer than walking, so a crowd goes round a hill it could
    /// have gone over — and cutting a gentle ramp into one is worth doing.
    pub fn step_cost(&self, from: TilePos, to: TilePos) -> Option<NonZeroU32> {
        let climb = self.height_at(to)? - self.height_at(from)?;
        if climb.abs() > Self::MAX_STEP {
            return None;
        }

        let crossing = self.ground(to)?.walk_cost()?;
        let steps = u32::from(climb.unsigned_abs());
        NonZeroU32::new(crossing + steps * Self::CLIMBING)
    }
}

#[cfg(test)]
mod tests {
    // Every float compared below is a whole number of steps, put there by an
    // integer conversion rather than arrived at by arithmetic.
    #![allow(clippy::float_cmp)]

    use super::*;

    fn flat_grass(size: u32) -> Land {
        Land::flat(Grid::filled(size, size, Terrain::Grass).unwrap()).unwrap()
    }

    #[test]
    fn new_land_is_flat_and_all_one_thing() {
        let land = flat_grass(8);
        assert_eq!(land.width(), 8);
        assert_eq!(land.height(), 8);
        assert_eq!(land.highest(), Land::MIN_HEIGHT);
        assert!(land
            .heights()
            .iter()
            .all(|(_, height)| *height == Land::MIN_HEIGHT));
    }

    #[test]
    fn a_tile_outside_the_park_has_no_shape_to_change() {
        let mut land = flat_grass(8);
        let outside = TilePos::new(-1, -1);

        assert_eq!(land.height_at(outside), None);
        assert_eq!(land.ground(outside), None);
        assert!(land.raise(outside).is_err());
        assert!(land.lower(outside).is_err());
        assert_eq!(land.set_ground(outside, Terrain::Path), None);
    }

    #[test]
    fn land_never_leaves_its_range() {
        let mut land = flat_grass(8);
        let tile = TilePos::new(2, 2);

        for _ in 0..Land::MAX_HEIGHT {
            land.raise(tile).unwrap();
        }
        assert_eq!(land.height_at(tile), Some(Land::MAX_HEIGHT));
        assert!(land.raise(tile).is_err(), "it went past the ceiling");

        for _ in 0..Land::MAX_HEIGHT {
            land.lower(tile).unwrap();
        }
        assert_eq!(land.height_at(tile), Some(Land::MIN_HEIGHT));
        assert!(land.lower(tile).is_err(), "it dug past the base");
    }

    #[test]
    fn the_highest_tile_is_the_one_that_was_raised() {
        let mut land = flat_grass(8);
        land.raise(TilePos::new(3, 3)).unwrap();
        land.raise(TilePos::new(3, 3)).unwrap();
        assert_eq!(land.highest(), 2);
    }

    #[test]
    fn a_gentle_step_costs_more_than_flat_ground() {
        let mut land = flat_grass(8);
        let (here, there) = (TilePos::new(1, 1), TilePos::new(1, 2));
        let flat = land.step_cost(here, there).unwrap();

        land.raise(there).unwrap();
        let uphill = land.step_cost(here, there).unwrap();
        assert!(uphill > flat, "climbing is free");

        // And coming back down is just as much work, which keeps a route over
        // a hill dearer than one around it in both directions.
        assert_eq!(land.step_cost(there, here).unwrap(), uphill);
    }

    #[test]
    fn nobody_walks_off_a_cliff() {
        let mut land = flat_grass(8);
        let (here, there) = (TilePos::new(1, 1), TilePos::new(1, 2));

        for _ in 0..=Land::MAX_STEP {
            land.raise(there).unwrap();
        }

        assert!(!land.is_a_step(here, there), "a cliff was climbed");
        assert!(!land.is_a_step(there, here), "a cliff was jumped off");
    }

    #[test]
    fn nobody_walks_into_the_water_however_flat_it_is() {
        let mut land = flat_grass(8);
        let there = TilePos::new(1, 2);
        land.set_ground(there, Terrain::Water);

        assert!(!land.is_a_step(TilePos::new(1, 1), there));
    }

    #[test]
    fn a_tile_is_drawn_at_the_height_it_stands() {
        let mut land = flat_grass(8);
        let tile = TilePos::new(4, 4);
        land.raise(tile).unwrap();

        assert_eq!(land.elevation(tile), 1.0);
        assert_eq!(land.point(tile).z, 1.0);
        assert_eq!(land.point(tile).x, tile.centre().x);
        assert_eq!(
            land.elevation(TilePos::new(-5, -5)),
            f32::from(Land::MIN_HEIGHT),
            "off the map reads as the base rather than refusing to draw"
        );
    }

    #[test]
    fn the_land_survives_a_save() {
        let mut land = flat_grass(8);
        land.raise(TilePos::new(1, 1)).unwrap();
        land.set_ground(TilePos::new(2, 2), Terrain::Path);

        let json = serde_json::to_string(&land).unwrap();
        assert_eq!(serde_json::from_str::<Land>(&json).unwrap(), land);
    }
    #[test]
    fn generated_hills_are_gentle_enough_to_walk_on() {
        for seed in 0..8 {
            let land = Land::rolling(
                Grid::filled(24, 24, Terrain::Grass).unwrap(),
                &mut Rng::from_seed(seed),
            )
            .unwrap();

            for (tile, _) in land.terrain().iter() {
                for beside in tile.neighbours() {
                    let Some(there) = land.height_at(beside) else {
                        continue;
                    };
                    let here = land.height_at(tile).unwrap();
                    assert!(
                        (here - there).abs() <= Land::MAX_STEP,
                        "seed {seed} left a cliff between {tile:?} and {beside:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn rolling_land_has_hills_in_it() {
        let land = Land::rolling(
            Grid::filled(24, 24, Terrain::Grass).unwrap(),
            &mut Rng::from_seed(3),
        )
        .unwrap();

        assert!(
            land.highest() > 0,
            "the generator produced a billiard table"
        );
    }

    #[test]
    fn water_and_paths_are_left_at_the_base() {
        let mut terrain = Grid::filled(24, 24, Terrain::Grass).unwrap();
        terrain.replace(TilePos::new(12, 12), Terrain::Water);
        terrain.replace(TilePos::new(4, 4), Terrain::Path);

        let land = Land::rolling(terrain, &mut Rng::from_seed(1)).unwrap();
        assert_eq!(land.height_at(TilePos::new(12, 12)), Some(0));
        assert_eq!(land.height_at(TilePos::new(4, 4)), Some(0));
    }
}
