//! The park itself: the land, the money, and the clock it all runs on.

mod books;
mod build;
mod crowd;
mod day;
mod facility;
#[cfg(test)]
mod fixtures;
mod guest;
mod land;
mod needs;
mod ride;
mod shop;
mod staff;
mod terrain;
mod track;
mod walk;

pub use facility::Facility;
pub use guest::{Guest, Plan};
pub use land::Land;
pub use needs::Needs;
pub use ride::{Ride, RideState, RideStats, TestFailure, Train};
pub use shop::Shop;
pub use staff::{Staff, StaffKind};
pub use terrain::Terrain;
pub use track::{Heading, Segment, Track, TrackPiece};
pub use walk::Walk;

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
    land: Land,
    facilities: Grid<Option<Shop>>,
    guests: Vec<Guest>,
    staff: Vec<Staff>,
    rides: Vec<Ride>,
    cash: Money,
    rng: Rng,
    tick: Tick,
    /// The id the next guest through the gate will get. Never reused, so that
    /// a guest can be followed across saves.
    next_guest_id: u32,
    /// How many guests have walked back out again.
    guests_who_left: u32,
    /// The id the next member of staff hired will get.
    next_staff_id: u32,
    /// The id the next ride built will get.
    next_ride_id: u32,
    /// When the park ran out of credit, if it has.
    bankrupt_since: Option<Tick>,
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

    /// How many ticks pass between one wage bill and the next.
    ///
    /// Thirty seconds at [`isogrid::time::TickRate::CLASSIC`]: often enough
    /// that a park full of staff and empty of guests is felt within a visit.
    pub const TICKS_PER_WAGE_BILL: u64 = 1_200;

    /// How far into debt a park is allowed to go before the bank closes it.
    pub const DEBT_LIMIT: Money = -5_000;

    /// What it costs to move one tile of land by one step.
    pub const LANDSCAPING: Money = 20;

    /// How many rides one park can hold.
    ///
    /// Every train's tick resolves its whole layout, so the cost of a park is
    /// bounded by this times [`Track::MAX_PIECES`].
    pub const MAX_RIDES: usize = 32;

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

    /// What a guest arrives with in its pocket, at the least and at the most.
    ///
    /// Enough for a few meals: a guest that runs out has to go home hungry,
    /// which is a fair outcome but should not be the usual one.
    const SPENDING_MONEY: (i32, i32) = (60, 200);

    /// How many of the nearest candidates a guest tries before deciding a
    /// facility is out of reach.
    const FACILITY_ATTEMPTS: usize = 4;

    /// The chance that one tick of a guest walking on grass wears it to dirt.
    ///
    /// Low enough that one guest crossing a lawn leaves it alone, high enough
    /// that the route everybody takes to the stall goes bare within a visit.
    const TRAMPLE_CHANCE: f32 = 0.002;

    /// How much mood a guest loses per tick of standing on worn-out ground.
    const DIRT_IS_DREARY: f32 = 1.0 / 4_000.0;

    /// How much mood a guest gains per tick of walking near an entertainer.
    const ENTERTAINED: f32 = 1.0 / 1_500.0;

    /// How many ticks a handyman takes to put one tile of dirt back to grass,
    /// and a mechanic to put one broken ride back together.
    const TICKS_PER_TIDY: u64 = 200;

    /// How likely a completely worn-out ride is to break down, per tick.
    ///
    /// Scaled by how worn the ride actually is, so a new one is as good as
    /// reliable and an old one fails every few thousand ticks.
    const BREAKDOWN_CHANCE: f32 = 0.0002;

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

        let facilities = Grid::filled(width, height, None)
            .context("the park is too small or too large for the engine to hold")?;

        let mut park = Self {
            name: name.into(),
            land: Land::rolling(terrain, &mut rng)?,
            facilities,
            guests: Vec::new(),
            staff: Vec::new(),
            rides: Vec::new(),
            cash: Self::STARTING_CASH,
            rng,
            tick: Tick::ZERO,
            next_guest_id: 0,
            guests_who_left: 0,
            next_staff_id: 0,
            next_ride_id: 0,
            bankrupt_since: None,
        };

        park.open_with_the_basics();
        Ok(park)
    }

    /// Puts up the handful of stalls and benches a new park comes with.
    ///
    /// They are a gift rather than a purchase: a park that opened with nothing
    /// at all would send its first guests home before its owner had finished
    /// looking around.
    fn open_with_the_basics(&mut self) {
        let (mid_x, mid_y) = (self.entrance().x, self.height() / 2);
        #[allow(clippy::cast_possible_wrap)]
        let mid_y = mid_y as i32;

        // Along the path down from the gate, alternating sides, so the first
        // thing a guest walks past is somewhere to eat.
        let plan = [
            (TilePos::new(mid_x - 1, mid_y / 2), Facility::FoodStall),
            (TilePos::new(mid_x + 1, mid_y / 2 + 3), Facility::Bench),
            (TilePos::new(mid_x + 1, mid_y - 2), Facility::FoodStall),
            (TilePos::new(mid_x - 1, mid_y + 2), Facility::Bench),
            (TilePos::new(mid_x - 1, mid_y + 6), Facility::Bench),
        ];

        for (tile, facility) in plan {
            // Ground that will not take it is simply left alone: on a small or
            // an unlucky map some of these land in the lake.
            let _ = self.put_up(tile, facility);
        }
    }

    /// The park's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The land: what the ground is made of and how high it stands.
    pub const fn land(&self) -> &Land {
        &self.land
    }

    /// What the ground is made of, tile by tile.
    pub const fn terrain(&self) -> &Grid<Terrain> {
        self.land.terrain()
    }

    /// What has been built on it.
    pub const fn facilities(&self) -> &Grid<Option<Shop>> {
        &self.facilities
    }

    /// What stands on one tile, if anything does.
    pub fn facility_at(&self, tile: TilePos) -> Option<Facility> {
        self.shop_at(tile).map(Shop::kind)
    }

    /// The shop standing on one tile, with its price and its till.
    pub fn shop_at(&self, tile: TilePos) -> Option<Shop> {
        self.facilities.get(tile).copied().flatten()
    }

    /// Everyone currently in the park.
    pub fn guests(&self) -> &[Guest] {
        &self.guests
    }

    /// How the park is going down with the people in it, from 0 to 1, or `None`
    /// when there is nobody to ask.
    ///
    /// ```
    /// # use openpark::park::Park;
    /// let mut park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// assert_eq!(park.average_happiness(), None, "an empty park has no opinion");
    ///
    /// for _ in 0..200 {
    ///     park.tick_once();
    /// }
    /// assert!(park.average_happiness().is_some());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn average_happiness(&self) -> Option<f32> {
        if self.guests.is_empty() {
            return None;
        }

        let total: f32 = self
            .guests
            .iter()
            .map(|guest| guest.needs().happiness())
            .sum();
        #[allow(clippy::cast_precision_loss)]
        let count = self.guests.len() as f32;
        Some(total / count)
    }

    /// How many guests have left since the park opened.
    pub const fn guests_who_left(&self) -> u32 {
        self.guests_who_left
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
        self.land.width()
    }

    /// The depth of the park in tiles.
    pub const fn height(&self) -> u32 {
        self.land.height()
    }

    /// Replaces the terrain of one tile, free of charge, returning what was
    /// there.
    ///
    /// Returns `None` and changes nothing if the tile is outside the park. This
    /// is the map generator's door in; [`Park::lay`] is the player's, and it
    /// charges.
    pub fn set_terrain(&mut self, tile: TilePos, terrain: Terrain) -> Option<Terrain> {
        self.land.set_ground(tile, terrain)
    }

    /// Every ride in the park.
    pub fn rides(&self) -> &[Ride] {
        &self.rides
    }

    /// One ride by its id.
    pub fn ride(&self, id: u32) -> Option<&Ride> {
        self.rides.iter().find(|ride| ride.id() == id)
    }

    /// Whichever ride has track on `tile`, if any has.
    pub fn ride_at(&self, tile: TilePos) -> Option<&Ride> {
        self.rides.iter().find(|ride| ride.track().occupies(tile))
    }
}

#[cfg(test)]
mod tests {
    use crate::park::fixtures::{bare_park_opened_for, opened_for, run, A_WHOLE_VISIT};

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
                assert_eq!(
                    park.facility_at(tile),
                    None,
                    "guest {} is standing inside a building",
                    guest.id()
                );
            }
        }
    }

    #[test]
    fn the_crowd_prefers_the_paths_to_the_grass() {
        let park = opened_for(3_000);

        // Measured per tile rather than per guest: a crossroads is a sliver of
        // the map, so a handful of guests standing on it is already a crowd,
        // and the lawn would win on headcount alone however unpopular it is.
        let crowding = |wanted: Terrain| {
            let tiles = park
                .terrain()
                .iter()
                .filter(|(_, ground)| **ground == wanted)
                .count();
            let guests = park
                .guests()
                .iter()
                .filter(|guest| park.terrain()[guest.tile()] == wanted)
                .count();

            #[allow(clippy::cast_precision_loss)]
            let crowding = guests as f32 / tiles.max(1) as f32;
            crowding
        };

        let on_a_path = crowding(Terrain::Path);
        let on_the_grass = crowding(Terrain::Grass);
        assert!(
            on_a_path > on_the_grass * 2.0,
            "the paths are {on_a_path} deep and the grass {on_the_grass}"
        );
    }

    #[test]
    fn the_crowd_wears_its_own_shortcuts_into_the_grass() {
        let park = opened_for(6_000);
        let worn = park
            .terrain()
            .iter()
            .filter(|(_, ground)| **ground == Terrain::Dirt)
            .count();

        assert!(
            worn > 0,
            "a park opens with no bare earth on it, so every patch of it was walked there"
        );
    }

    #[test]
    fn guests_wear_down_as_they_walk_the_park() {
        let park = opened_for(2_000);
        let guest = &park.guests()[0];
        assert!(guest.needs().hunger() > 0.0, "nobody worked up an appetite");
        assert!(guest.needs().energy() < 1.0, "nobody's feet hurt");
    }

    #[test]
    fn an_empty_park_has_no_opinion_of_itself() {
        let park = Park::new("Empty", 16, 16, 0).unwrap();
        assert_eq!(park.average_happiness(), None);
    }

    #[test]
    fn the_average_mood_sits_between_the_moods_it_averages() {
        let park = opened_for(2_000);
        let happiness = park.average_happiness().expect("there is a crowd to ask");

        let moods: Vec<f32> = park
            .guests()
            .iter()
            .map(|guest| guest.needs().happiness())
            .collect();
        let lowest = moods.iter().copied().fold(f32::INFINITY, f32::min);
        let highest = moods.iter().copied().fold(f32::NEG_INFINITY, f32::max);

        assert!(happiness >= lowest && happiness <= highest);
    }

    #[test]
    fn a_park_with_nothing_in_it_sends_its_guests_home() {
        let park = bare_park_opened_for(A_WHOLE_VISIT);
        assert!(park.guests_who_left() > 0, "nobody ever left");
    }

    #[test]
    fn a_park_with_stalls_and_benches_keeps_its_guests_happier() {
        let fed = opened_for(A_WHOLE_VISIT);
        let starved = bare_park_opened_for(A_WHOLE_VISIT);

        // Not a count of who left: with nothing to ride, everybody eventually
        // gets bored and goes home from either park, and boredom swamps the
        // difference. What a stall buys a park is a happier crowd while they
        // are in it. A ride is what buys their time, and that is asserted of a
        // park that has one.
        assert!(
            fed.average_happiness() > starved.average_happiness(),
            "the crowd was no happier for being fed"
        );
    }

    #[test]
    fn the_stalls_take_money_off_the_guests() {
        let park = opened_for(A_WHOLE_VISIT);
        let inside = Money::try_from(park.guests().len()).expect("a countable crowd");
        let admissions = (inside + Money::from(park.guests_who_left())) * Park::ADMISSION;

        // The park's starting stalls were a gift rather than a purchase, so
        // anything above the takings at the gate came over a counter.
        assert!(
            park.cash() > Park::STARTING_CASH + admissions,
            "the park took {admissions} on the gate and nothing over the counter"
        );
    }

    #[test]
    fn guests_actually_eat_and_sit_down() {
        let mut park = Park::new("Hungry", 32, 32, 5).unwrap();
        let mut seen_using = false;

        for _ in 0..A_WHOLE_VISIT {
            park.tick_once();
            if park
                .guests()
                .iter()
                .any(|guest| matches!(guest.plan(), Plan::Using { .. }))
            {
                seen_using = true;
                break;
            }
        }

        assert!(seen_using, "nobody ever stopped at a stall or a bench");
    }

    #[test]
    fn guests_leave_through_the_gate_and_not_over_the_fence() {
        let mut park = Park::new("Leaving", 32, 32, 5).unwrap();
        for tile in park.terrain().positions().collect::<Vec<_>>() {
            park.demolish(tile);
        }
        let mut seen_leaving = false;

        for _ in 0..A_WHOLE_VISIT {
            let before = park.guests().len();
            park.tick_once();

            // Anyone who left must have been standing at the gate to do it.
            if park.guests().len() < before {
                seen_leaving = true;
                // A guest still walking through the gate on a longer route is
                // fine; one that is going home, has run out of route, and is
                // standing on the gate should have been shown out.
                assert!(
                    park.guests().iter().all(|guest| {
                        !(guest.is_going_home()
                            && guest.is_idle()
                            && guest.tile() == park.entrance())
                    }),
                    "somebody was left standing at the gate"
                );
            }
        }

        assert!(
            seen_leaving,
            "nobody left in a whole visit's worth of ticks"
        );
    }

    #[test]
    fn only_the_fed_up_go_home() {
        let park = bare_park_opened_for(A_WHOLE_VISIT / 4);
        for guest in park.guests() {
            assert!(
                !guest.is_going_home() || guest.needs().is_fed_up(),
                "guest {} is leaving in a perfectly good mood",
                guest.id()
            );
        }
    }

    #[test]
    fn the_gate_keeps_working_after_people_start_leaving() {
        let park = bare_park_opened_for(A_WHOLE_VISIT * 2);
        assert!(park.guests_who_left() > 0);
        assert!(!park.guests().is_empty(), "the park emptied out for good");
        assert!(
            park.cash() > Park::STARTING_CASH + Park::ADMISSION,
            "the turnstile stopped taking money"
        );
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

    #[test]
    fn the_crowd_walks_round_a_cliff_rather_than_over_it() {
        let mut park = Park::new("Blocked", 32, 32, 5).unwrap();

        // A pillar in the middle of the crossroads, too steep to climb.
        let blocked = TilePos::new(16, 16);
        assert_eq!(park.terrain()[blocked], Terrain::Path);
        park.adjust_cash(10_000);
        for _ in 0..=Land::MAX_STEP {
            park.raise(blocked).unwrap();
        }

        let park = run(park, 3_000);
        assert!(
            park.guests().iter().all(|guest| guest.tile() != blocked),
            "somebody climbed a cliff"
        );
        assert!(!park.guests().is_empty(), "the park emptied out instead");
    }

    #[test]
    fn a_new_park_has_hills_in_it_but_flat_paths() {
        let park = Park::new("Rolling", 32, 32, 5).unwrap();
        let land = park.land();

        assert!(land.highest() > 0, "a new park came out flat");
        assert!(
            land.terrain()
                .iter()
                .filter(|(_, ground)| matches!(ground, Terrain::Path | Terrain::Water))
                .all(|(tile, _)| land.height_at(tile) == Some(0)),
            "a path or the lake climbed a hill"
        );
    }

    #[test]
    fn the_same_seed_rolls_the_same_hills() {
        let one = Park::new("Rolling", 32, 32, 9).unwrap();
        let two = Park::new("Rolling", 32, 32, 9).unwrap();
        let other = Park::new("Rolling", 32, 32, 10).unwrap();

        assert_eq!(one.land().heights(), two.land().heights());
        assert_ne!(
            one.land().heights(),
            other.land().heights(),
            "every seed rolled the same landscape"
        );
    }
}
