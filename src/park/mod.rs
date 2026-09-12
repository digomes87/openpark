//! The park itself: the land, the money, and the clock it all runs on.

mod facility;
mod guest;
mod needs;
mod shop;
mod staff;
mod terrain;
mod walk;

pub use facility::Facility;
pub use guest::{Guest, Plan};
pub use needs::Needs;
pub use shop::Shop;
pub use staff::{Staff, StaffKind};
pub use terrain::Terrain;
pub use walk::Walk;

use core::num::NonZeroU32;

use anyhow::{Context, Result};
use isogrid::grid::{Grid, TileBounds};
use isogrid::iso::TilePos;
use isogrid::path::{PathFinder, Traversable};
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
    facilities: Grid<Option<Shop>>,
    guests: Vec<Guest>,
    staff: Vec<Staff>,
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

    /// How many ticks a handyman takes to put one tile of dirt back to grass.
    const TICKS_PER_TIDY: u64 = 200;

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
            terrain,
            facilities,
            guests: Vec::new(),
            staff: Vec::new(),
            cash: Self::STARTING_CASH,
            rng,
            tick: Tick::ZERO,
            next_guest_id: 0,
            guests_who_left: 0,
            next_staff_id: 0,
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

    /// The land.
    pub const fn terrain(&self) -> &Grid<Terrain> {
        &self.terrain
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

    /// Sets what the shop on `tile` charges, returning the price on the board.
    ///
    /// # Errors
    ///
    /// Fails if there is nothing on that tile to price.
    ///
    /// ```
    /// # use openpark::park::{Facility, Park, Shop};
    /// # use isogrid::iso::TilePos;
    /// let mut park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// let tile = TilePos::new(2, 2);
    /// park.build(tile, Facility::FoodStall)?;
    ///
    /// assert_eq!(park.set_price(tile, 18)?, 18);
    /// assert_eq!(park.set_price(tile, 10_000)?, Shop::MAX_PRICE, "clamped");
    /// assert!(park.set_price(TilePos::new(3, 3), 5).is_err(), "nothing there");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn set_price(&mut self, tile: TilePos, price: Money) -> Result<Money> {
        self.repricing(tile, |shop| shop.set_price(price))
    }

    /// Puts the price on `tile` up by one step.
    ///
    /// # Errors
    ///
    /// Fails if there is nothing on that tile to price.
    pub fn raise_price(&mut self, tile: TilePos) -> Result<Money> {
        self.repricing(tile, Shop::raise_price)
    }

    /// Brings the price on `tile` down by one step.
    ///
    /// # Errors
    ///
    /// Fails if there is nothing on that tile to price.
    pub fn lower_price(&mut self, tile: TilePos) -> Result<Money> {
        self.repricing(tile, Shop::lower_price)
    }

    /// Changes the price of whatever is on `tile`, however it is being changed.
    fn repricing(
        &mut self,
        tile: TilePos,
        change: impl FnOnce(&mut Shop) -> Money,
    ) -> Result<Money> {
        let shop = self
            .facilities
            .get_mut(tile)
            .with_context(|| format!("{tile:?} is outside the park"))?
            .as_mut()
            .with_context(|| format!("there is nothing on {tile:?} to put a price on"))?;

        Ok(change(shop))
    }

    /// Builds a facility, taking its cost out of the bank.
    ///
    /// # Errors
    ///
    /// Fails if the tile is outside the park, the ground will not take it,
    /// something is already there, or the park cannot afford it. Nothing is
    /// changed when it fails.
    ///
    /// ```
    /// # use openpark::park::{Facility, Park};
    /// # use isogrid::iso::TilePos;
    /// let mut park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// let before = park.cash();
    ///
    /// let tile = TilePos::new(2, 2);
    /// park.build(tile, Facility::Bench)?;
    /// assert_eq!(park.facility_at(tile), Some(Facility::Bench));
    /// assert_eq!(park.cash(), before - Facility::Bench.build_cost());
    ///
    /// assert!(park.build(tile, Facility::Bench).is_err(), "it is taken");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn build(&mut self, tile: TilePos, facility: Facility) -> Result<()> {
        self.check_build(tile, facility)?;
        self.facilities.replace(tile, Some(Shop::new(facility)));
        self.adjust_cash(-facility.build_cost());
        Ok(())
    }

    /// Whether [`Park::build`] would succeed, for a cursor that wants to say so
    /// before the click rather than after it.
    pub fn can_build(&self, tile: TilePos, facility: Facility) -> bool {
        self.check_build(tile, facility).is_ok()
    }

    /// Why a facility cannot be bought for a tile, as an error worth showing.
    fn check_build(&self, tile: TilePos, facility: Facility) -> Result<()> {
        anyhow::ensure!(
            !self.is_bankrupt(),
            "the park is bankrupt and cannot buy anything",
        );
        anyhow::ensure!(
            self.cash >= facility.build_cost(),
            "a {} costs {} and the park has {}",
            facility.name(),
            facility.build_cost(),
            self.cash,
        );

        self.check_ground(tile, facility)
    }

    /// Why a tile will not take a facility, money aside.
    fn check_ground(&self, tile: TilePos, facility: Facility) -> Result<()> {
        let ground = *self
            .terrain
            .get(tile)
            .with_context(|| format!("{tile:?} is outside the park"))?;

        anyhow::ensure!(
            ground.is_buildable(),
            "a {} cannot be built on {ground:?}",
            facility.name(),
        );
        anyhow::ensure!(
            self.facility_at(tile).is_none(),
            "there is already something on {tile:?}",
        );

        Ok(())
    }

    /// Puts a facility up without charging for it.
    fn put_up(&mut self, tile: TilePos, facility: Facility) -> Result<()> {
        self.check_ground(tile, facility)?;
        self.facilities.replace(tile, Some(Shop::new(facility)));
        Ok(())
    }

    /// Takes a facility down again, returning what was there.
    ///
    /// Nothing comes back for it: a demolished stall is a loss, which is what
    /// makes building one a decision.
    pub fn demolish(&mut self, tile: TilePos) -> Option<Shop> {
        self.facilities.replace(tile, None).flatten()
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

    /// Everybody the park is paying.
    pub fn staff(&self) -> &[Staff] {
        &self.staff
    }

    /// Takes somebody on, charging the one-off cost of hiring them.
    ///
    /// They start at the gate, like everybody else, and walk in from there.
    /// Returns the id they were given, so they can be fired again later.
    ///
    /// # Errors
    ///
    /// Fails if the park cannot afford the hiring cost, or is bankrupt.
    ///
    /// ```
    /// # use openpark::park::{Park, StaffKind};
    /// let mut park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// let before = park.cash();
    ///
    /// let id = park.hire(StaffKind::Handyman)?;
    /// assert_eq!(park.staff().len(), 1);
    /// assert_eq!(park.cash(), before - StaffKind::Handyman.hire_cost());
    /// assert!(park.fire(id).is_some());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn hire(&mut self, kind: StaffKind) -> Result<u32> {
        anyhow::ensure!(
            !self.is_bankrupt(),
            "the park is bankrupt and cannot take anybody on",
        );
        anyhow::ensure!(
            self.cash >= kind.hire_cost(),
            "hiring a {} costs {} and the park has {}",
            kind.name().to_lowercase(),
            kind.hire_cost(),
            self.cash,
        );

        let id = self.next_staff_id;
        self.next_staff_id = self.next_staff_id.wrapping_add(1);
        self.staff.push(Staff::hired(id, kind, self.entrance()));
        self.adjust_cash(-kind.hire_cost());
        Ok(id)
    }

    /// Lets somebody go, returning who left. No severance: they walk off the
    /// map, and the wage bill is lighter from the next one on.
    pub fn fire(&mut self, id: u32) -> Option<Staff> {
        let at = self.staff.iter().position(|staff| staff.id() == id)?;
        Some(self.staff.remove(at))
    }

    /// Lets go of whoever is standing on `tile`, for a pointer that has one of
    /// them under it rather than an id.
    pub fn fire_at(&mut self, tile: TilePos) -> Option<Staff> {
        let at = self.staff.iter().position(|staff| staff.tile() == tile)?;
        Some(self.staff.remove(at))
    }

    /// What the park owes every [`Park::TICKS_PER_WAGE_BILL`]: every wage, plus
    /// the upkeep of everything standing on the land.
    ///
    /// ```
    /// # use openpark::park::{Park, StaffKind};
    /// let mut park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// let before = park.wage_bill();
    ///
    /// park.hire(StaffKind::Entertainer)?;
    /// assert_eq!(park.wage_bill(), before + StaffKind::Entertainer.wage());
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn wage_bill(&self) -> Money {
        let wages: Money = self.staff.iter().map(Staff::wage).sum();
        let upkeep: Money = self
            .facilities
            .iter()
            .filter_map(|(_, built)| built.as_ref())
            .map(|shop| shop.upkeep())
            .sum();

        wages.saturating_add(upkeep)
    }

    /// Everything every till has taken since the park opened.
    pub fn takings(&self) -> Money {
        self.facilities
            .iter()
            .filter_map(|(_, built)| built.as_ref())
            .map(|shop| shop.takings())
            .sum()
    }

    /// Whether the bank has closed the park.
    ///
    /// A bankrupt park keeps its land and its buildings, and keeps drawing
    /// them, but nobody new comes through the gate, everybody inside heads for
    /// it, and nothing more can be bought.
    pub const fn is_bankrupt(&self) -> bool {
        self.bankrupt_since.is_some()
    }

    /// When the park went bankrupt, if it has.
    pub const fn bankrupt_since(&self) -> Option<Tick> {
        self.bankrupt_since
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

        // A closed park has nobody at the gate to sell a ticket.
        if !self.is_bankrupt() {
            self.admit_a_guest();
        }

        self.walk_the_guests();
        self.walk_the_staff();
        self.do_the_rounds();

        if self.tick.is_multiple_of(Self::TICKS_PER_WAGE_BILL) {
            self.pay_the_bills();
            self.check_solvency();
        }

        self.show_out_the_guests();
    }

    /// Pays every wage and every bit of upkeep, once a wage bill is due.
    fn pay_the_bills(&mut self) {
        let bill = self.wage_bill();
        if bill == 0 {
            return;
        }

        self.adjust_cash(-bill);
        tracing::debug!(bill, cash = self.cash, "the park paid its bills");
    }

    /// Closes the park if it has run past [`Park::DEBT_LIMIT`].
    ///
    /// Bankruptcy is one-way: there is no coming back from it inside a game,
    /// only starting another park.
    fn check_solvency(&mut self) {
        if self.is_bankrupt() || self.cash >= Self::DEBT_LIMIT {
            return;
        }

        self.bankrupt_since = Some(self.tick);
        tracing::warn!(
            cash = self.cash,
            tick = self.tick.get(),
            "the park is bankrupt"
        );
        for guest in &mut self.guests {
            guest.decide(Plan::GoingHome);
        }
    }

    /// Moves every member of staff one tick's worth along their route, and
    /// gives them somewhere new to be when they run out of one.
    ///
    /// Staff wander for now: a handyman with a round to walk and a queue to
    /// prioritise is a job for the day there are rides to break down.
    fn walk_the_staff(&mut self) {
        if self.staff.is_empty() {
            return;
        }

        let Self {
            terrain,
            facilities,
            staff,
            rng,
            ..
        } = self;

        let map = ParkMap {
            terrain,
            facilities,
        };
        let mut finder = PathFinder::new();

        for member in staff.iter_mut() {
            if member.is_idle() {
                if let Some(route) = wander(&map, rng, &mut finder, member.tile()) {
                    if let Err(error) = member.follow(route) {
                        tracing::warn!(%error, staff = member.id(), "ignoring an impossible route");
                    }
                }
            }

            member.advance(Self::speed_across(terrain[member.tile()]));
        }
    }

    /// Lets every member of staff do the job they are paid for.
    fn do_the_rounds(&mut self) {
        if self.staff.is_empty() {
            return;
        }

        // Copied out first: the work changes the guests and the land, both of
        // which the staff themselves are standing in.
        let rounds: Vec<(StaffKind, TilePos)> = self
            .staff
            .iter()
            .map(|member| (member.kind(), member.tile()))
            .collect();
        let tidying = self.tick.is_multiple_of(Self::TICKS_PER_TIDY);

        for (kind, at) in rounds {
            match kind {
                StaffKind::Entertainer => {
                    for guest in &mut self.guests {
                        if at.manhattan_distance(guest.tile()) <= kind.reach() {
                            guest.cheer_up(Self::ENTERTAINED);
                        }
                    }
                }
                StaffKind::Handyman => {
                    if tidying {
                        self.tidy_up_around(at, kind.reach());
                    }
                }
            }
        }
    }

    /// Puts one tile of worn ground within `reach` of `at` back to grass.
    ///
    /// One tile per visit rather than all of them: a single handyman cannot
    /// keep up with a crowd, which is what makes hiring a second one a
    /// decision.
    fn tidy_up_around(&mut self, at: TilePos, reach: u32) {
        #[allow(clippy::cast_possible_wrap)]
        let span = reach as i32;

        for dy in -span..=span {
            for dx in -span..=span {
                let tile = at.offset(dx, dy);
                if at.manhattan_distance(tile) > reach {
                    continue;
                }

                if self.terrain.get(tile) == Some(&Terrain::Dirt) {
                    self.terrain.replace(tile, Terrain::Grass);
                    return;
                }
            }
        }
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
        let (least, most) = Self::SPENDING_MONEY;
        let money = Money::from(self.rng.range(least, most));
        let guest = Guest::arriving(self.next_guest_id, self.entrance(), shirt, money);

        self.next_guest_id = self.next_guest_id.wrapping_add(1);
        self.guests.push(guest);
        self.adjust_cash(Self::ADMISSION);
    }

    /// Moves every guest one tick's worth along its route, and lets it act on
    /// what it wants: eat, sit down, wander, or give up and go home.
    fn walk_the_guests(&mut self) {
        let entrance = self.entrance();
        let now = self.tick;
        let closed = self.is_bankrupt();

        // Everything the guests do to the park is collected as it happens and
        // applied afterwards: the land and the tills are borrowed out to the
        // pathfinder for the length of the walk.
        let (takings, sales, trampled) = {
            // Destructured so that the borrow checker can see the guests, the
            // land, what is built on it and the dice as separate things.
            let Self {
                terrain,
                facilities,
                guests,
                rng,
                ..
            } = self;

            let map = ParkMap {
                terrain,
                facilities,
            };

            // Allocates nothing until a guest actually needs a route, and reuses
            // its buffers across everyone who does.
            let mut finder = PathFinder::new();
            let mut takings = 0;
            let mut sales: Vec<(TilePos, Money)> = Vec::new();
            let mut trampled: Vec<TilePos> = Vec::new();

            for guest in guests.iter_mut() {
                guest.live();

                // A park the bank has closed cannot talk anybody into staying.
                if closed {
                    guest.decide(Plan::GoingHome);
                }

                // Someone in the middle of a meal or a sit down is busy.
                if let Plan::Using { facility, until } = guest.plan() {
                    if now < until {
                        continue;
                    }

                    if let Some(shop) = facilities.get(facility).copied().flatten() {
                        let paid = guest.enjoy(&shop);
                        if paid > 0 {
                            takings += paid;
                            sales.push((facility, paid));
                        }
                    }
                    guest.decide(Plan::Wandering);
                }

                if guest.needs().is_fed_up() {
                    guest.decide(Plan::GoingHome);
                }

                let here = guest.tile();
                let ground = terrain[here];

                // Worn-out ground is a shabby thing to walk across.
                if ground == Terrain::Dirt {
                    guest.put_off(Self::DIRT_IS_DREARY);
                }

                if !guest.is_idle() {
                    // And the walking is what wears it out in the first place.
                    if ground == Terrain::Grass
                        && wears_from_here(terrain, here)
                        && rng.chance(Self::TRAMPLE_CHANCE)
                    {
                        trampled.push(here);
                    }

                    guest.advance(Self::speed_across(ground));
                    continue;
                }

                // Arrived next to what it came for: stop and use it.
                if let Plan::Visiting { facility } = guest.plan() {
                    if guest.tile().neighbours().contains(&facility) {
                        if let Some(shop) = facilities.get(facility).copied().flatten() {
                            guest.decide(Plan::Using {
                                facility,
                                until: now.after(shop.kind().ticks_to_use()),
                            });
                            continue;
                        }
                    }
                }

                let route = match Self::what_next(guest, &map, rng, &mut finder, entrance) {
                    Some((plan, route)) => {
                        guest.decide(plan);
                        Some(route)
                    }
                    None => None,
                };

                if let Some(route) = route {
                    if let Err(error) = guest.follow(route) {
                        // Only reachable if the pathfinder returned a route
                        // starting somewhere other than where it was asked to.
                        tracing::warn!(%error, guest = guest.id(), "ignoring an impossible route");
                    }
                }

                guest.advance(Self::speed_across(terrain[guest.tile()]));
            }

            (takings, sales, trampled)
        };

        for (tile, amount) in sales {
            if let Some(Some(shop)) = self.facilities.get_mut(tile) {
                shop.take(amount);
            }
        }

        for tile in trampled {
            self.terrain.replace(tile, Terrain::Dirt);
        }

        self.adjust_cash(takings);
    }

    /// Decides what an idle guest does next, and how it gets there.
    ///
    /// Returns `None` when nowhere it wants to go can be reached this tick,
    /// which leaves the guest standing and trying again on the next one.
    fn what_next(
        guest: &Guest,
        map: &ParkMap<'_>,
        rng: &mut Rng,
        finder: &mut PathFinder,
        entrance: TilePos,
    ) -> Option<(Plan, Vec<TilePos>)> {
        let from = guest.tile();

        if guest.is_going_home() {
            return Some((
                Plan::GoingHome,
                finder.find(map, from, entrance)?.tiles().to_vec(),
            ));
        }

        if let Some(wanted) = Self::what_it_wants(guest) {
            if let Some((facility, route)) = nearest_facility(map, finder, from, wanted, guest) {
                return Some((Plan::Visiting { facility }, route));
            }
        }

        Some((Plan::Wandering, wander(map, rng, finder, from)?))
    }

    /// What the guest would go out of its way for, if anything.
    ///
    /// Hunger first: it is the need that ends a visit. What the guest can
    /// afford, and what it thinks is a fair price, is settled per shop by
    /// [`nearest_facility`] — two stalls in one park need not agree on either.
    fn what_it_wants(guest: &Guest) -> Option<Facility> {
        if guest.needs().wants_food() {
            return Some(Facility::FoodStall);
        }
        if guest.needs().wants_a_sit_down() {
            return Some(Facility::Bench);
        }
        None
    }

    /// Sees off everyone who has made it back to the gate.
    fn show_out_the_guests(&mut self) {
        let entrance = self.entrance();
        let before = self.guests.len();

        self.guests.retain(|guest| {
            !(guest.is_going_home() && guest.is_idle() && guest.tile() == entrance)
        });

        let left = before - self.guests.len();
        if left > 0 {
            self.guests_who_left = self
                .guests_who_left
                .saturating_add(u32::try_from(left).unwrap_or(u32::MAX));
            tracing::debug!(left, remaining = self.guests.len(), "guests went home");
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

/// The park as the pathfinder sees it: ground that can be crossed, minus
/// whatever has been built on it.
///
/// A facility blocks its own tile, which is what makes guests queue beside a
/// stall rather than walk through it.
struct ParkMap<'a> {
    terrain: &'a Grid<Terrain>,
    facilities: &'a Grid<Option<Shop>>,
}

impl Traversable for ParkMap<'_> {
    fn bounds(&self) -> TileBounds {
        self.terrain.bounds()
    }

    fn step_cost(&self, _from: TilePos, to: TilePos) -> Option<NonZeroU32> {
        if self.facilities.get(to)?.is_some() {
            return None;
        }
        NonZeroU32::new(self.terrain.get(to)?.walk_cost()?)
    }
}

/// Whether a tile of grass is somewhere wear would actually start.
///
/// Only grass next to a path or to ground already worn down: wear spreads from
/// the edges of where people are already walking, which is what turns it into
/// trails across the lawn rather than a rash of bare patches all over it.
fn wears_from_here(terrain: &Grid<Terrain>, tile: TilePos) -> bool {
    tile.neighbours()
        .iter()
        .any(|beside| matches!(terrain.get(*beside), Some(Terrain::Path | Terrain::Dirt)))
}

/// Finds the nearest facility of a kind that a guest at `from` can actually
/// walk up to and will pay for, and the route to the tile it would stand on.
///
/// Only the [`Park::FACILITY_ATTEMPTS`] nearest are tried: past that the walk
/// is long enough that the guest may as well wander and ask again later.
/// Anything charging more than the guest thinks it is worth is not a candidate
/// at all — an overpriced stall is invisible rather than disappointing.
fn nearest_facility(
    map: &ParkMap<'_>,
    finder: &mut PathFinder,
    from: TilePos,
    wanted: Facility,
    guest: &Guest,
) -> Option<(TilePos, Vec<TilePos>)> {
    let mut candidates: Vec<(u32, TilePos)> = map
        .facilities
        .iter()
        .filter_map(|(tile, built)| {
            let shop = (*built)?;
            (shop.kind() == wanted && guest.will_pay(&shop))
                .then_some((from.manhattan_distance(tile), tile))
        })
        .collect();
    candidates.sort_unstable();

    for (_, facility) in candidates.into_iter().take(Park::FACILITY_ATTEMPTS) {
        // The counter is beside the stall, never on it.
        for beside in facility.neighbours() {
            if let Some(route) = finder.find(map, from, beside) {
                return Some((facility, route.tiles().to_vec()));
            }
        }
    }

    None
}

/// Picks somewhere for a guest at `from` to go, and works out how to get there.
///
/// Returns `None` if nowhere reachable turned up in [`Park::WANDER_ATTEMPTS`]
/// tries, which leaves the guest standing for a tick and trying again on the
/// next one — cheaper than searching a whole park for the one open tile.
fn wander(
    map: &ParkMap<'_>,
    rng: &mut Rng,
    finder: &mut PathFinder,
    from: TilePos,
) -> Option<Vec<TilePos>> {
    for attempt in 0..Park::WANDER_ATTEMPTS {
        #[allow(clippy::cast_possible_wrap)]
        let goal = TilePos::new(
            rng.below(map.terrain.width())? as i32,
            rng.below(map.terrain.height())? as i32,
        );

        if goal == from {
            continue;
        }

        let ground = map.terrain[goal];
        let wanted = if attempt < Park::PATH_ATTEMPTS {
            ground == Terrain::Path
        } else {
            ground.is_walkable()
        };

        if wanted {
            if let Some(route) = finder.find(map, from, goal) {
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
        run(Park::new("Busy", 32, 32, 5).unwrap(), ticks)
    }

    /// Runs a park that has had everything torn down, so that guests have
    /// nowhere to eat and nowhere to sit.
    fn bare_park_opened_for(ticks: u64) -> Park {
        let mut park = Park::new("Bare", 32, 32, 5).unwrap();
        for tile in park.terrain().positions().collect::<Vec<_>>() {
            park.demolish(tile);
        }
        assert!(park.facilities().iter().all(|(_, built)| built.is_none()));
        run(park, ticks)
    }

    fn run(mut park: Park, ticks: u64) -> Park {
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
    fn rough_ground_slows_a_guest_down() {
        assert!(Park::speed_across(Terrain::Path) > Park::speed_across(Terrain::Grass));
        assert!(Park::speed_across(Terrain::Grass) > 0.0);
        // Impassable ground is never stood on, but a speed of zero there would
        // strand anyone the terrain changed underneath.
        assert!(Park::speed_across(Terrain::Water) > 0.0);
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

    /// How long a guest lasts in a park with nothing in it, near enough.
    const A_WHOLE_VISIT: u64 = 20_000;

    #[test]
    fn a_park_with_nothing_in_it_sends_its_guests_home() {
        let park = bare_park_opened_for(A_WHOLE_VISIT);
        assert!(park.guests_who_left() > 0, "nobody ever left");
    }

    #[test]
    fn a_park_with_stalls_and_benches_keeps_its_guests() {
        let fed = opened_for(A_WHOLE_VISIT);
        let starved = bare_park_opened_for(A_WHOLE_VISIT);

        assert!(
            fed.guests_who_left() < starved.guests_who_left(),
            "feeding the guests made no difference: {} left either way",
            fed.guests_who_left()
        );
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
    fn a_cursor_can_ask_before_it_clicks() {
        let mut park = Park::new("Building", 32, 32, 5).unwrap();
        let grass = park
            .terrain()
            .positions()
            .find(|tile| park.terrain()[*tile].is_buildable() && park.facility_at(*tile).is_none())
            .expect("there is bare ground somewhere");

        assert!(park.can_build(grass, Facility::Bench));
        park.build(grass, Facility::Bench).unwrap();
        assert!(!park.can_build(grass, Facility::Bench), "it is taken now");

        let water = park
            .terrain()
            .positions()
            .find(|tile| park.terrain()[*tile] == Terrain::Water)
            .expect("this park has a lake");
        assert!(!park.can_build(water, Facility::Bench));
        assert!(!park.can_build(TilePos::new(999, 999), Facility::Bench));
    }

    #[test]
    fn a_park_that_cannot_pay_cannot_build() {
        let mut park = Park::new("Broke", 32, 32, 5).unwrap();
        park.adjust_cash(-park.cash());

        let grass = park
            .terrain()
            .positions()
            .find(|tile| park.terrain()[*tile].is_buildable() && park.facility_at(*tile).is_none())
            .expect("there is bare ground somewhere");

        assert!(!park.can_build(grass, Facility::FoodStall));
        let refused = park.build(grass, Facility::FoodStall).unwrap_err();
        assert!(refused.to_string().contains("costs"), "{refused}");
        assert_eq!(park.facility_at(grass), None);
    }

    #[test]
    fn demolishing_gives_the_ground_back() {
        let mut park = Park::new("Clearing", 32, 32, 5).unwrap();
        let built = park
            .facilities()
            .iter()
            .find_map(|(tile, facility)| facility.map(|facility| (tile, facility)))
            .expect("a new park comes with something on it");

        assert_eq!(park.demolish(built.0), Some(built.1));
        assert_eq!(park.demolish(built.0), None, "it was already gone");
        assert!(park.can_build(built.0, Facility::Bench));
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
    fn the_bills_come_out_of_the_bank() {
        let mut park = Park::new("Overheads", 32, 32, 5).unwrap();
        for tile in park.terrain().positions().collect::<Vec<_>>() {
            park.demolish(tile);
        }
        park.hire(StaffKind::Handyman).unwrap();
        let bill = park.wage_bill();
        assert_eq!(bill, StaffKind::Handyman.wage(), "nothing else is standing");

        let before = park.cash();
        let park = run(park, Park::TICKS_PER_WAGE_BILL);
        assert_eq!(
            park.cash(),
            before + Park::ADMISSION * 20 - bill,
            "twenty tickets sold and one wage bill paid"
        );
    }

    #[test]
    fn upkeep_is_owed_on_everything_standing() {
        let park = Park::new("Overheads", 32, 32, 5).unwrap();
        let standing: Money = park
            .facilities()
            .iter()
            .filter_map(|(_, built)| built.as_ref())
            .map(|shop| shop.upkeep())
            .sum();

        assert!(standing > 0, "a new park opens with something on it");
        assert_eq!(park.wage_bill(), standing, "and nobody on the payroll");
    }

    #[test]
    fn a_park_that_runs_out_of_credit_goes_bankrupt() {
        let mut park = Park::new("Broke", 32, 32, 5).unwrap();
        assert!(!park.is_bankrupt(), "a park opens solvent");

        // Deep enough that a wage bill's worth of ticket sales cannot climb
        // back out of it before the bank looks.
        park.adjust_cash(Park::DEBT_LIMIT * 2 - park.cash());
        let park = run(park, Park::TICKS_PER_WAGE_BILL);

        assert!(park.is_bankrupt(), "the bank let {} through", park.cash());
        assert_eq!(
            park.bankrupt_since(),
            Some(Tick::new(Park::TICKS_PER_WAGE_BILL))
        );
    }

    /// A park already past its debt limit, closed by the bank.
    fn bankrupt_park() -> Park {
        let mut park = Park::new("Broke", 32, 32, 5).unwrap();
        park.adjust_cash(Park::DEBT_LIMIT * 2 - park.cash());
        let park = run(park, Park::TICKS_PER_WAGE_BILL);
        assert!(park.is_bankrupt());
        park
    }

    #[test]
    fn a_bankrupt_park_sells_no_more_tickets() {
        let park = bankrupt_park();
        let inside = park.guests().len();
        let cash = park.cash();

        let park = run(park, Park::TICKS_BETWEEN_ARRIVALS * 4);
        assert!(park.guests().len() <= inside, "somebody got in anyway");
        assert!(park.cash() <= cash, "somebody paid at the gate");
    }

    #[test]
    fn a_bankrupt_park_sends_everybody_home() {
        let park = bankrupt_park();
        assert!(
            park.guests().iter().all(Guest::is_going_home),
            "somebody is still enjoying themselves"
        );

        let park = run(park, A_WHOLE_VISIT);
        assert!(park.guests().is_empty(), "the park never emptied out");
    }

    #[test]
    fn a_bankrupt_park_cannot_buy_anything() {
        let mut park = bankrupt_park();
        park.adjust_cash(100_000);

        assert!(
            park.build(TilePos::new(3, 3), Facility::Bench).is_err(),
            "a bankrupt park went shopping"
        );
        assert!(park.hire(StaffKind::Handyman).is_err());
    }

    #[test]
    fn nobody_pays_over_the_odds() {
        let mut park = Park::new("Rip Off", 32, 32, 5).unwrap();
        let stalls: Vec<TilePos> = park
            .facilities()
            .iter()
            .filter(|(_, built)| built.is_some_and(|shop| shop.kind() == Facility::FoodStall))
            .map(|(tile, _)| tile)
            .collect();
        assert!(!stalls.is_empty(), "there is nothing to overcharge for");

        for tile in stalls {
            park.set_price(tile, Shop::MAX_PRICE).unwrap();
        }

        let park = run(park, A_WHOLE_VISIT);
        assert_eq!(park.takings(), 0, "somebody paid {}", Shop::MAX_PRICE);
    }

    #[test]
    fn a_fair_price_fills_the_till() {
        let park = opened_for(A_WHOLE_VISIT);
        assert!(park.takings() > 0, "nobody bought anything all day");

        let customers: u32 = park
            .facilities()
            .iter()
            .filter_map(|(_, built)| built.as_ref())
            .map(|shop| shop.customers())
            .sum();
        assert!(customers > 0, "the till took money from nobody");
    }

    #[test]
    fn only_something_that_is_standing_there_can_be_priced() {
        let mut park = Park::new("Prices", 32, 32, 1).unwrap();
        let tile = TilePos::new(2, 2);
        park.build(tile, Facility::FoodStall).unwrap();

        assert_eq!(
            park.raise_price(tile).unwrap(),
            Facility::FoodStall.price() + Shop::PRICE_STEP
        );
        assert_eq!(park.lower_price(tile).unwrap(), Facility::FoodStall.price());
        assert!(
            park.raise_price(TilePos::new(3, 3)).is_err(),
            "nothing there"
        );
        assert!(
            park.set_price(TilePos::new(-1, -1), 5).is_err(),
            "outside the park"
        );
    }

    #[test]
    fn a_demolished_shop_takes_its_till_with_it() {
        let mut park = Park::new("Closing Down", 32, 32, 1).unwrap();
        let tile = TilePos::new(2, 2);
        park.build(tile, Facility::FoodStall).unwrap();
        park.set_price(tile, 30).unwrap();

        let bill = park.wage_bill();
        let gone = park.demolish(tile).expect("something was there");

        assert_eq!(gone.price(), 30, "the till went with it");
        assert_eq!(park.shop_at(tile), None);
        assert_eq!(
            park.wage_bill(),
            bill - gone.upkeep(),
            "the park is still paying to run something it tore down"
        );
    }

    #[test]
    fn firing_somebody_takes_them_off_the_payroll() {
        let mut park = Park::new("Payroll", 32, 32, 1).unwrap();
        let id = park.hire(StaffKind::Handyman).unwrap();
        let bill = park.wage_bill();

        let gone = park.fire(id).expect("somebody was hired");
        assert_eq!(gone.id(), id);
        assert_eq!(park.wage_bill(), bill - StaffKind::Handyman.wage());
        assert!(park.fire(id).is_none(), "fired twice");
        assert!(park.staff().is_empty());
    }

    #[test]
    fn somebody_can_be_let_go_by_the_tile_they_are_standing_on() {
        let mut park = Park::new("Payroll", 32, 32, 1).unwrap();
        park.hire(StaffKind::Entertainer).unwrap();
        let at = park.staff()[0].tile();

        assert!(park.fire_at(TilePos::new(-1, -1)).is_none());
        assert!(park.fire_at(at).is_some());
        assert!(park.staff().is_empty());
    }

    #[test]
    fn a_handyman_puts_the_worn_ground_back() {
        let mut park = Park::new("Tidy", 32, 32, 5).unwrap();
        park.hire(StaffKind::Handyman).unwrap();
        let at = park.staff()[0].tile();

        // Worn ground right under their feet, so the round reaches it.
        let worn: Vec<TilePos> = at
            .neighbours()
            .into_iter()
            .filter(|tile| park.terrain().contains(*tile))
            .collect();
        for tile in &worn {
            park.set_terrain(*tile, Terrain::Dirt);
        }
        let before = dirt(&park);
        assert!(before >= worn.len(), "the dirt was not laid down");

        let park = run(park, Park::TICKS_PER_TIDY * 2);
        assert!(
            dirt(&park) < before,
            "the handyman left {} tiles of dirt alone",
            dirt(&park)
        );
    }

    /// How much of the park is worn down to bare earth.
    fn dirt(park: &Park) -> usize {
        park.terrain()
            .iter()
            .filter(|(_, ground)| **ground == Terrain::Dirt)
            .count()
    }

    #[test]
    fn an_entertainer_cheers_the_crowd_up() {
        let plain = bare_park_opened_for(A_WHOLE_VISIT / 4);

        let mut entertained = Park::new("Bare", 32, 32, 5).unwrap();
        for tile in entertained.terrain().positions().collect::<Vec<_>>() {
            entertained.demolish(tile);
        }
        entertained.hire(StaffKind::Entertainer).unwrap();
        let entertained = run(entertained, A_WHOLE_VISIT / 4);

        let (Some(with), Some(without)) =
            (entertained.average_happiness(), plain.average_happiness())
        else {
            panic!("both parks should have somebody in them");
        };
        assert!(
            with > without,
            "an entertainer left the crowd at {with} against {without}"
        );
    }

    #[test]
    fn a_park_survives_a_save_with_its_staff_and_its_prices() {
        let mut park = opened_for(2_000);
        park.hire(StaffKind::Handyman).unwrap();
        park.hire(StaffKind::Entertainer).unwrap();
        let priced = park
            .facilities()
            .iter()
            .find_map(|(tile, built)| built.map(|_| tile))
            .expect("something is standing");
        park.set_price(priced, 17).unwrap();

        let json = serde_json::to_string(&park).unwrap();
        let loaded: Park = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded, park);
        assert_eq!(loaded.shop_at(priced).map(Shop::price), Some(17));
        assert_eq!(loaded.staff().len(), 2);
    }
}
