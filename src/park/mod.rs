//! The park itself: the land, the money, and the clock it all runs on.

mod guest;
mod terrain;

pub use guest::Guest;
pub use terrain::Terrain;

use core::num::NonZeroU32;

use anyhow::{Context, Result};
use isogrid::grid::Grid;
use isogrid::iso::TilePos;
use isogrid::path::{with_cost, PathFinder};
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
    guests: Vec<Guest>,
    cash: Money,
    rng: Rng,
    tick: Tick,
    /// The id the next guest through the gate will get. Never reused, so that
    /// a guest can be followed across saves.
    next_guest_id: u32,
}

impl Park {
    /// The smallest park worth having, in tiles per side.
    pub const MIN_SIZE: u32 = 8;

    /// What a new park starts with in the bank.
    pub const STARTING_CASH: Money = 10_000;

    /// What a guest pays at the gate.
    pub const ADMISSION: Money = 20;

    /// How many guests the park holds before the queue outside stops moving.
    pub const CAPACITY: usize = 120;

    /// How many ticks pass between arrivals.
    ///
    /// At [`isogrid::time::TickRate::CLASSIC`] that is a guest every second and
    /// a half, so a new park fills up over a few minutes rather than all at
    /// once.
    const TICKS_BETWEEN_ARRIVALS: u64 = 60;

    /// How far a guest walks each tick on open path, in tiles.
    ///
    /// One tile a second at the classic tick rate.
    const WALK_SPEED: f32 = 1.0 / 40.0;

    /// How much each extra point of [`Terrain::walk_cost`] slows a guest down.
    const ROUGH_GROUND_PENALTY: f32 = 0.25;

    /// How many goals a wandering guest considers before giving up for a tick.
    const WANDER_ATTEMPTS: u32 = 8;

    /// How many of those attempts insist on a goal that is actually a path.
    ///
    /// Wanting to end up on a path is what keeps the crowd on the paths: the
    /// route between two path tiles is itself nearly all path, because a path
    /// step costs a sixth of a step across the grass.
    const PATH_ATTEMPTS: u32 = 6;

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
            guests: Vec::new(),
            cash: Self::STARTING_CASH,
            rng,
            tick: Tick::ZERO,
            next_guest_id: 0,
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

    /// Everyone currently in the park.
    pub fn guests(&self) -> &[Guest] {
        &self.guests
    }

    /// The tile guests arrive on: where the path down the middle meets the
    /// northern edge.
    ///
    /// ```
    /// # use openpark::park::{Park, Terrain};
    /// let park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// assert!(park.terrain()[park.entrance()].is_walkable());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn entrance(&self) -> TilePos {
        #[allow(clippy::cast_possible_wrap)]
        TilePos::new((self.width() / 2) as i32, 0)
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
    /// The clock moves, someone may come through the gate, and everybody
    /// already inside takes a step. Rides come later; this is the loop they
    /// will hang off.
    ///
    /// ```
    /// # use openpark::park::Park;
    /// let mut park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// for _ in 0..200 {
    ///     park.tick_once();
    /// }
    /// assert!(!park.guests().is_empty(), "nobody turned up");
    /// assert!(park.cash() > Park::STARTING_CASH, "nobody paid to get in");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn tick_once(&mut self) {
        self.tick = self.tick.after(1);
        self.admit_a_guest();
        self.walk_the_guests();
    }

    /// Lets one guest in, if one is due and there is room.
    fn admit_a_guest(&mut self) {
        if !self.tick.is_multiple_of(Self::TICKS_BETWEEN_ARRIVALS)
            || self.guests.len() >= Self::CAPACITY
        {
            return;
        }

        #[allow(clippy::cast_possible_truncation)]
        let shirt = self.rng.next_u32() as u8;
        let guest = Guest::arriving(self.next_guest_id, self.entrance(), shirt);

        self.next_guest_id = self.next_guest_id.wrapping_add(1);
        self.guests.push(guest);
        self.adjust_cash(Self::ADMISSION);
    }

    /// Moves every guest one tick's worth along its route, finding a new route
    /// for anyone who has arrived where they were going.
    fn walk_the_guests(&mut self) {
        // Destructured so that the borrow checker can see the guests, the land
        // and the dice as three separate things.
        let Self {
            terrain,
            guests,
            rng,
            ..
        } = self;

        // Allocates nothing until a guest actually needs a route, and reuses
        // its buffers across everyone who does.
        let mut finder = PathFinder::new();

        for guest in guests.iter_mut() {
            if guest.is_idle() {
                if let Some(route) = wander(terrain, rng, &mut finder, guest.tile()) {
                    if let Err(error) = guest.follow(route) {
                        // Only reachable if the pathfinder returned a route
                        // starting somewhere other than where it was asked to.
                        tracing::warn!(%error, guest = guest.id(), "ignoring an impossible route");
                    }
                }
            }

            guest.advance(Self::speed_across(terrain[guest.tile()]));
        }
    }

    /// How far a guest standing on `terrain` moves in one tick.
    fn speed_across(terrain: Terrain) -> f32 {
        let cost = terrain.walk_cost().unwrap_or(1);
        #[allow(clippy::cast_precision_loss)]
        let slowdown = (cost - 1) as f32 * Self::ROUGH_GROUND_PENALTY;
        Self::WALK_SPEED / (1.0 + slowdown)
    }
}

/// Picks somewhere for a guest at `from` to go, and works out how to get there.
///
/// Returns `None` if nowhere reachable turned up in [`Park::WANDER_ATTEMPTS`]
/// tries, which leaves the guest standing for a tick and trying again on the
/// next one — cheaper than searching a whole park for the one open tile.
fn wander(
    terrain: &Grid<Terrain>,
    rng: &mut Rng,
    finder: &mut PathFinder,
    from: TilePos,
) -> Option<Vec<TilePos>> {
    let map = with_cost(terrain, |ground: &Terrain| {
        NonZeroU32::new(ground.walk_cost()?)
    });

    for attempt in 0..Park::WANDER_ATTEMPTS {
        #[allow(clippy::cast_possible_wrap)]
        let goal = TilePos::new(
            rng.below(terrain.width())? as i32,
            rng.below(terrain.height())? as i32,
        );

        if goal == from {
            continue;
        }

        let ground = terrain[goal];
        let wanted = if attempt < Park::PATH_ATTEMPTS {
            ground == Terrain::Path
        } else {
            ground.is_walkable()
        };

        if wanted {
            if let Some(route) = finder.find(&map, from, goal) {
                return Some(route.tiles().to_vec());
            }
        }
    }

    None
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
    fn ticking_advances_the_clock_and_leaves_the_land_alone() {
        let mut park = Park::new("Ticking", 8, 8, 0).unwrap();
        let before = park.clone();
        park.tick_once();
        assert_eq!(park.tick().get(), before.tick().get() + 1);
        assert_eq!(park.terrain(), before.terrain());
        assert_eq!(park.cash(), before.cash(), "the gate opened too early");
        assert!(park.guests().is_empty());
    }

    /// Runs a park for `ticks` ticks and hands it back.
    fn opened_for(ticks: u64) -> Park {
        let mut park = Park::new("Busy", 32, 32, 5).unwrap();
        for _ in 0..ticks {
            park.tick_once();
        }
        park
    }

    #[test]
    fn guests_arrive_at_the_gate_and_pay_to_get_in() {
        let park = opened_for(Park::TICKS_BETWEEN_ARRIVALS);
        assert_eq!(park.guests().len(), 1);
        assert_eq!(park.cash(), Park::STARTING_CASH + Park::ADMISSION);
        assert_eq!(park.guests()[0].tile(), park.entrance());
    }

    #[test]
    fn guests_keep_arriving_at_a_steady_rate() {
        let park = opened_for(Park::TICKS_BETWEEN_ARRIVALS * 5);
        assert_eq!(park.guests().len(), 5);
        assert_eq!(park.cash(), Park::STARTING_CASH + Park::ADMISSION * 5);
    }

    #[test]
    fn the_park_stops_letting_people_in_when_it_is_full() {
        let ticks = Park::TICKS_BETWEEN_ARRIVALS * (Park::CAPACITY as u64 + 10);
        let park = opened_for(ticks);
        assert_eq!(park.guests().len(), Park::CAPACITY);
    }

    #[test]
    fn every_guest_has_an_id_of_their_own() {
        let park = opened_for(Park::TICKS_BETWEEN_ARRIVALS * 12);
        let mut ids: Vec<_> = park.guests().iter().map(Guest::id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), park.guests().len());
    }

    #[test]
    fn guests_walk_away_from_the_gate() {
        let park = opened_for(Park::TICKS_BETWEEN_ARRIVALS + 400);
        let wandered = park
            .guests()
            .iter()
            .any(|guest| guest.tile() != park.entrance());
        assert!(wandered, "everybody is still standing in the doorway");
    }

    #[test]
    fn a_guest_never_stands_somewhere_it_could_not_walk() {
        let mut park = Park::new("Wandering", 32, 32, 11).unwrap();
        for _ in 0..2_000 {
            park.tick_once();
            for guest in park.guests() {
                let tile = guest.tile();
                assert!(
                    park.terrain().contains(tile),
                    "guest {} left the park at {tile:?}",
                    guest.id()
                );
                assert!(
                    park.terrain()[tile].is_walkable(),
                    "guest {} is standing in {:?}",
                    guest.id(),
                    park.terrain()[tile]
                );
            }
        }
    }

    #[test]
    fn the_crowd_prefers_the_paths_to_the_grass() {
        let park = opened_for(3_000);
        let on_a_path = park
            .guests()
            .iter()
            .filter(|guest| park.terrain()[guest.tile()] == Terrain::Path)
            .count();
        assert!(
            on_a_path * 2 > park.guests().len(),
            "only {on_a_path} of {} guests stuck to the paths",
            park.guests().len()
        );
    }

    #[test]
    fn rough_ground_slows_a_guest_down() {
        assert!(Park::speed_across(Terrain::Path) > Park::speed_across(Terrain::Grass));
        assert!(Park::speed_across(Terrain::Grass) > 0.0);
        // Impassable ground is never stood on, but a speed of zero there would
        // strand anyone the terrain changed underneath.
        assert!(Park::speed_across(Terrain::Water) > 0.0);
    }

    #[test]
    fn the_same_seed_runs_the_same_park() {
        let a = opened_for(500);
        let b = opened_for(500);
        assert_eq!(a, b, "two identical parks diverged");
    }

    #[test]
    fn a_park_full_of_guests_survives_a_save() {
        let park = opened_for(500);
        assert!(!park.guests().is_empty());

        let json = serde_json::to_string(&park).unwrap();
        assert_eq!(serde_json::from_str::<Park>(&json).unwrap(), park);
    }
}
