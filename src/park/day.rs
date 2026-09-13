//! One tick of a day in the park.
//!
//! The order things happen in, and the passes themselves: the gate, the crowd,
//! the trains, the payroll and the books. Everything here changes the park, and
//! everything it needs to make a decision it asks [`super::crowd`] for.

use isogrid::iso::TilePos;
use isogrid::path::PathFinder;
use isogrid::rng::Rng;

use crate::park::crowd::{
    nearest_facility, nearest_ride, wander, wears_from_here, ParkMap, Wanted,
};
use crate::park::{Facility, Guest, Money, Park, Plan, Ride, RideState, StaffKind, Terrain};

impl Park {
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
        self.load_the_trains();
        self.run_the_rides();
        self.wear_and_tear();
        self.walk_the_staff();
        self.do_the_rounds();

        if self.tick.is_multiple_of(Self::TICKS_PER_WAGE_BILL) {
            self.pay_the_bills();
            self.check_solvency();
        }

        self.show_out_the_guests();
    }

    /// Puts whoever is waiting beside a station onto the train that is loading.
    ///
    /// A guest pays as it boards and the park keeps the fare, which is why the
    /// ride's till and the bank move together.
    fn load_the_trains(&mut self) {
        let mut fares = 0;

        for index in 0..self.guests.len() {
            let Plan::Queueing { ride, station } = self.guests[index].plan() else {
                continue;
            };

            // Still walking there, or the queue moved and it is not beside the
            // station yet.
            if !self.guests[index].tile().neighbours().contains(&station) {
                continue;
            }

            let Ok(at) = self.index_of_ride(ride) else {
                // The ride was taken down while somebody was walking to it.
                self.guests[index].decide(Plan::Wandering);
                continue;
            };

            if self.rides[at].boarding().is_none() {
                // Shut, broken, or the train is out on the circuit: wait.
                if !self.rides[at].is_open() {
                    self.guests[index].decide(Plan::Wandering);
                }
                continue;
            }

            let price = self.rides[at].price();
            if !self.guests[index].can_afford(price) {
                self.guests[index].decide(Plan::Wandering);
                continue;
            }

            if let Some(fare) = self.rides[at].board_one() {
                fares += self.guests[index].pay(fare);
                self.guests[index].decide_anyway(Plan::Riding { ride });
            }
        }

        self.adjust_cash(fares);
    }

    /// Runs every ride for a tick, and puts whoever got off back on their feet.
    fn run_the_rides(&mut self) {
        for at in 0..self.rides.len() {
            let got_off = self.rides[at].tick();
            if got_off == 0 {
                continue;
            }

            let id = self.rides[at].id();
            let (excitement, intensity) = self.rides[at]
                .stats()
                .map_or((0.0, 0.0), |stats| (stats.excitement, stats.intensity));

            let mut left = got_off;
            for guest in &mut self.guests {
                if left == 0 {
                    break;
                }
                if guest.riding() == Some(id) {
                    guest.enjoy_a_ride(excitement, intensity);
                    guest.get_off();
                    left -= 1;
                }
            }
        }
    }

    /// Gives every running ride its chance to break down.
    ///
    /// The chance rises with how worn the ride is, so a new one almost never
    /// fails and an old one that nobody has repaired fails often. A broken ride
    /// waits for a mechanic, and a park with none never runs again.
    fn wear_and_tear(&mut self) {
        for at in 0..self.rides.len() {
            if !self.rides[at].is_open() {
                continue;
            }

            let wear = self.rides[at].wear();
            if wear > 0.0 && self.rng.chance(wear * Self::BREAKDOWN_CHANCE) {
                self.rides[at].break_down();
                tracing::info!(ride = %self.rides[at].name(), wear, "a ride broke down");
            }
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
            land,
            facilities,
            staff,
            rng,
            ..
        } = self;

        let map = ParkMap { land, facilities };
        let mut finder = PathFinder::new();

        for member in staff.iter_mut() {
            if member.is_idle() {
                if let Some(route) = wander(&map, rng, &mut finder, member.tile()) {
                    if let Err(error) = member.follow(route) {
                        tracing::warn!(%error, staff = member.id(), "ignoring an impossible route");
                    }
                }
            }

            let standing_on = land.ground(member.tile()).unwrap_or(Terrain::Grass);
            member.advance(Self::speed_across(standing_on));
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
                StaffKind::Mechanic => {
                    if tidying {
                        self.fix_something_around(at, kind.reach());
                    }
                }
            }
        }
    }

    /// Puts one broken ride within `reach` of `at` back into service.
    ///
    /// The nearest one by its track rather than by its station: a mechanic
    /// walking past the back of a ride can still get at the mechanism.
    fn fix_something_around(&mut self, at: TilePos, reach: u32) {
        let broken = self.rides.iter().position(|ride| {
            ride.state() == RideState::Broken
                && ride
                    .track()
                    .tiles()
                    .iter()
                    .any(|tile| at.manhattan_distance(*tile) <= reach)
        });

        let Some(index) = broken else {
            return;
        };

        self.rides[index].repair();
        let name = self.rides[index].name().to_owned();
        match self.rides[index].open() {
            Ok(()) => tracing::info!(ride = %name, "a mechanic put a ride back into service"),
            Err(error) => tracing::warn!(%error, ride = %name, "a repaired ride would not open"),
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

                if self.land.ground(tile) == Some(Terrain::Dirt) {
                    self.land.set_ground(tile, Terrain::Grass);
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
                land,
                facilities,
                guests,
                rides,
                rng,
                ..
            } = self;

            let map = ParkMap { land, facilities };

            // Allocates nothing until a guest actually needs a route, and reuses
            // its buffers across everyone who does.
            let mut finder = PathFinder::new();
            let mut takings = 0;
            let mut sales: Vec<(TilePos, Money)> = Vec::new();
            let mut trampled: Vec<TilePos> = Vec::new();

            for guest in guests.iter_mut() {
                guest.live();

                // Aboard something: not on the map until the ride brings it
                // back, so it neither walks nor wears the grass out.
                if guest.riding().is_some() {
                    continue;
                }

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
                let ground = land.ground(here).unwrap_or(Terrain::Grass);

                // Worn-out ground is a shabby thing to walk across.
                if ground == Terrain::Dirt {
                    guest.put_off(Self::DIRT_IS_DREARY);
                }

                if !guest.is_idle() {
                    // And the walking is what wears it out in the first place.
                    if ground == Terrain::Grass
                        && wears_from_here(land, here)
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

                let route = match Self::what_next(guest, &map, rng, &mut finder, entrance, rides) {
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

                let standing_on = land.ground(guest.tile()).unwrap_or(Terrain::Grass);
                guest.advance(Self::speed_across(standing_on));
            }

            (takings, sales, trampled)
        };

        for (tile, amount) in sales {
            if let Some(Some(shop)) = self.facilities.get_mut(tile) {
                shop.take(amount);
            }
        }

        for tile in trampled {
            self.land.set_ground(tile, Terrain::Dirt);
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
        rides: &[Ride],
    ) -> Option<(Plan, Vec<TilePos>)> {
        let from = guest.tile();

        if guest.is_going_home() {
            return Some((
                Plan::GoingHome,
                finder.find(map, from, entrance)?.tiles().to_vec(),
            ));
        }

        match Self::what_it_wants(guest) {
            Some(Wanted::Ride) => {
                if let Some((ride, station, route)) = nearest_ride(map, finder, from, rides, guest)
                {
                    return Some((Plan::Queueing { ride, station }, route));
                }
            }
            Some(Wanted::Something(kind)) => {
                if let Some((facility, route)) = nearest_facility(map, finder, from, kind, guest) {
                    return Some((Plan::Visiting { facility }, route));
                }
            }
            None => {}
        }

        Some((Plan::Wandering, wander(map, rng, finder, from)?))
    }

    /// What the guest would go out of its way for, if anything.
    ///
    /// Hunger first: it is the need that ends a visit. What the guest can
    /// afford, and what it thinks is a fair price, is settled per shop by
    /// [`nearest_facility`] — two stalls in one park need not agree on either.
    fn what_it_wants(guest: &Guest) -> Option<Wanted> {
        // Something to do comes first — it is what the guest came for — but a
        // guest that is starving or footsore sorts that out before queueing.
        if guest.needs().wants_food() {
            return Some(Wanted::Something(Facility::FoodStall));
        }
        if guest.needs().wants_a_sit_down() {
            return Some(Wanted::Something(Facility::Bench));
        }
        if guest.needs().wants_a_ride() {
            return Some(Wanted::Ride);
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

#[cfg(test)]
mod tests {
    use super::*;

    use isogrid::iso::TilePos;

    #[test]
    fn rough_ground_slows_a_guest_down() {
        assert!(Park::speed_across(Terrain::Path) > Park::speed_across(Terrain::Grass));
        assert!(Park::speed_across(Terrain::Grass) > 0.0);
        // Impassable ground is never stood on, but a speed of zero there would
        // strand anyone the terrain changed underneath.
        assert!(Park::speed_across(Terrain::Water) > 0.0);
    }

    #[test]
    fn a_single_step_is_a_ramp_and_two_is_a_wall() {
        let mut park = Park::new("Ramped", 32, 32, 5).unwrap();
        let ramp = TilePos::new(16, 16);
        let below = TilePos::new(16, 15);
        assert_eq!(park.terrain()[ramp], Terrain::Path, "on the crossroads");

        // A step up is dearer to cross than flat ground — that is what sends a
        // crowd round it — but it is still a route.
        park.raise(ramp).unwrap();
        let mut finder = PathFinder::new();
        {
            let map = ParkMap {
                land: &park.land,
                facilities: &park.facilities,
            };
            assert!(
                finder.find(&map, below, ramp).is_some(),
                "one step turned out to be a wall"
            );
        }

        // A second one is a cliff, and a cliff is not a route.
        park.raise(ramp).unwrap();
        let map = ParkMap {
            land: &park.land,
            facilities: &park.facilities,
        };
        assert!(
            finder.find(&map, below, ramp).is_none(),
            "somebody climbed two steps at once"
        );
    }
}
