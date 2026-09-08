//! The park itself: the land, the money, and the clock it all runs on.

mod terrain;

pub use terrain::Terrain;

use anyhow::{Context, Result};
use isogrid::grid::Grid;
use isogrid::iso::TilePos;
use isogrid::rng::Rng;
use isogrid::time::Tick;
use serde::{Deserialize, Serialize};

/// Money, in whole units of the park's currency.
///
/// A signed integer, because a park can absolutely go into debt, and because
/// floating point money is how rounding errors become gameplay.
pub type Money = i64;

/// Everything about one park.
///
/// This is the whole save file: given the same `Park` and the same number of
/// ticks, the simulation produces the same result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Park {
    name: String,
    terrain: Grid<Terrain>,
    cash: Money,
    rng: Rng,
    tick: Tick,
}

impl Park {
    /// The smallest park worth having, in tiles per side.
    pub const MIN_SIZE: u32 = 8;

    /// What a new park starts with in the bank.
    pub const STARTING_CASH: Money = 10_000;

    /// Builds a new park of open grass with a crossroads of path through it.
    ///
    /// # Errors
    ///
    /// Fails if either dimension is below [`Park::MIN_SIZE`], or if the size is
    /// one the engine will not allocate.
    ///
    /// ```
    /// # use openpark::park::{Park, Terrain};
    /// # use isogrid::iso::TilePos;
    /// let park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// assert_eq!(park.cash(), Park::STARTING_CASH);
    /// assert_eq!(park.terrain()[TilePos::new(16, 16)], Terrain::Path);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn new(name: impl Into<String>, width: u32, height: u32, seed: u64) -> Result<Self> {
        anyhow::ensure!(
            width >= Self::MIN_SIZE && height >= Self::MIN_SIZE,
            "a park must be at least {size}x{size} tiles, got {width}x{height}",
            size = Self::MIN_SIZE,
        );

        let mut rng = Rng::from_seed(seed);
        let (mid_x, mid_y) = (width / 2, height / 2);

        let terrain = Grid::from_fn(width, height, |tile| {
            let (x, y) = (tile.x.unsigned_abs(), tile.y.unsigned_abs());

            // A crossroads through the middle: somewhere for the first guests
            // to arrive on, and something to build against.
            if x == mid_x || y == mid_y {
                return Terrain::Path;
            }

            // A lake in one quadrant, so the map is not a featureless field and
            // the pathfinder has something to route around.
            let (dx, dy) = (x.abs_diff(width / 4), y.abs_diff(height / 4));
            if dx * dx + dy * dy < (width.min(height) / 8).pow(2) {
                return Terrain::Water;
            }

            // A little scattered rock, deterministic from the seed.
            if rng.chance(0.02) {
                Terrain::Rock
            } else {
                Terrain::Grass
            }
        })
        .context("the park is too small or too large for the engine to hold")?;

        Ok(Self {
            name: name.into(),
            terrain,
            cash: Self::STARTING_CASH,
            rng,
            tick: Tick::ZERO,
        })
    }

    /// The park's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The land.
    pub const fn terrain(&self) -> &Grid<Terrain> {
        &self.terrain
    }

    /// How much money is in the bank.
    pub const fn cash(&self) -> Money {
        self.cash
    }

    /// How many ticks the park has been open.
    pub const fn tick(&self) -> Tick {
        self.tick
    }

    /// The width of the park in tiles.
    pub const fn width(&self) -> u32 {
        self.terrain.width()
    }

    /// The height of the park in tiles.
    pub const fn height(&self) -> u32 {
        self.terrain.height()
    }

    /// Adds to or subtracts from the bank balance.
    ///
    /// Saturates rather than overflowing: a park deep enough in debt to wrap a
    /// 64-bit integer has other problems.
    ///
    /// ```
    /// # use openpark::park::Park;
    /// let mut park = Park::new("Test", 8, 8, 0)?;
    /// park.adjust_cash(-500);
    /// assert_eq!(park.cash(), Park::STARTING_CASH - 500);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn adjust_cash(&mut self, amount: Money) {
        self.cash = self.cash.saturating_add(amount);
    }

    /// Replaces the terrain of one tile, returning what was there.
    ///
    /// Returns `None` and changes nothing if the tile is outside the park.
    pub fn set_terrain(&mut self, tile: TilePos, terrain: Terrain) -> Option<Terrain> {
        self.terrain.replace(tile, terrain)
    }

    /// Advances the park by exactly one tick.
    ///
    /// Nothing lives here yet beyond the clock — guests and rides arrive in
    /// later work. It exists now so the loop it belongs to is real from the
    /// start rather than retrofitted.
    pub fn tick_once(&mut self) {
        self.tick = self.tick.after(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_park_must_be_big_enough_to_be_worth_playing() {
        assert!(Park::new("Too small", 4, 32, 0).is_err());
        assert!(Park::new("Too small", 32, 4, 0).is_err());
        assert!(Park::new("Just right", 8, 8, 0).is_ok());
    }

    #[test]
    fn a_new_park_has_a_crossroads_of_path() {
        let park = Park::new("Crossroads", 20, 20, 0).unwrap();
        assert_eq!(park.terrain()[TilePos::new(10, 10)], Terrain::Path);
        assert_eq!(park.terrain()[TilePos::new(10, 3)], Terrain::Path);
        assert_eq!(park.terrain()[TilePos::new(3, 10)], Terrain::Path);
    }

    #[test]
    fn the_path_is_never_flooded_or_blocked() {
        // The lake and the rocks are generated after the path, so this checks
        // that they never overwrite it.
        for seed in 0..32 {
            let park = Park::new("Flooded?", 24, 24, seed).unwrap();
            for tile in park.terrain().positions() {
                if tile.x == 12 || tile.y == 12 {
                    assert_eq!(
                        park.terrain()[tile],
                        Terrain::Path,
                        "seed {seed} built over the path at {tile:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_same_seed_builds_the_same_park() {
        let a = Park::new("Twin", 32, 32, 7).unwrap();
        let b = Park::new("Twin", 32, 32, 7).unwrap();
        assert_eq!(a, b);

        let different = Park::new("Twin", 32, 32, 8).unwrap();
        assert_ne!(a, different, "two seeds produced identical land");
    }

    #[test]
    fn a_park_survives_a_save() {
        let park = Park::new("Saved", 16, 16, 3).unwrap();
        let json = serde_json::to_string(&park).unwrap();
        assert_eq!(serde_json::from_str::<Park>(&json).unwrap(), park);
    }

    #[test]
    fn cash_saturates_instead_of_wrapping() {
        let mut park = Park::new("Rich", 8, 8, 0).unwrap();
        park.adjust_cash(Money::MAX);
        park.adjust_cash(Money::MAX);
        assert_eq!(park.cash(), Money::MAX);

        park.adjust_cash(Money::MIN);
        park.adjust_cash(Money::MIN);
        assert_eq!(park.cash(), Money::MIN);
    }

    #[test]
    fn terrain_outside_the_park_cannot_be_changed() {
        let mut park = Park::new("Edges", 8, 8, 0).unwrap();
        assert_eq!(park.set_terrain(TilePos::new(99, 99), Terrain::Path), None);
        assert!(park.set_terrain(TilePos::ORIGIN, Terrain::Path).is_some());
        assert_eq!(park.terrain()[TilePos::ORIGIN], Terrain::Path);
    }

    #[test]
    fn ticking_advances_the_clock_and_nothing_else() {
        let mut park = Park::new("Ticking", 8, 8, 0).unwrap();
        let before = park.clone();
        park.tick_once();
        assert_eq!(park.tick().get(), before.tick().get() + 1);
        assert_eq!(park.terrain(), before.terrain());
        assert_eq!(park.cash(), before.cash());
    }
}
