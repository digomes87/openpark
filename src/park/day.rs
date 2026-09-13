//! One tick of a day in the park.
//!
//! The order things happen in, and the passes themselves: the gate, the crowd,
//! the trains, the payroll and the books. Everything here changes the park, and
//! everything it needs to make a decision it asks [`super::crowd`] for.

use isogrid::iso::TilePos;
use isogrid::path::PathFinder;
use isogrid::rng::Rng;
use isogrid::time::Tick;

use crate::park::crowd::{
    nearest_facility, nearest_ride, wander, wears_from_here, ParkMap, Wanted,
};
use crate::park::queue;
use crate::park::{
    Facility, Guest, Money, Park, Plan, Queue, Rating, Ride, RideState, StaffKind, Terrain,
};

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
        self.work_the_queues();
        self.run_the_rides();
        self.wear_and_tear();
        self.walk_the_staff();
        self.do_the_rounds();

        if self.tick.is_multiple_of(Self::TICKS_PER_RATING) {
            self.reputation = self.rating().rating;
        }

        if self.tick.is_multiple_of(Self::TICKS_PER_WAGE_BILL) {
            self.pay_the_bills();
            self.check_solvency();
        }

        self.show_out_the_guests();
    }

    /// Moves every line along: joins, shuffles forward, boards, and gives up.
    ///
    /// The order matters. Somebody who has just arrived joins the back before
    /// the line shuffles, so they walk to their place in the same tick; the
    /// front boards last, so a train that is loading takes whoever has actually
    /// reached the front rather than whoever is nearest to it.
    fn work_the_queues(&mut self) {
        self.clear_out_the_lines();
        self.join_the_queues();
        self.shuffle_the_queues();
        self.load_the_trains();
        self.give_up_waiting();
    }

    /// Drops anybody from a line who is no longer waiting in it.
    ///
    /// A guest that gives up on the day decides to go home, and a guest that has
    /// decided to go home does not change its mind — so without this it keeps
    /// its place at the front of a queue it is walking away from, and nobody
    /// behind it ever boards.
    fn clear_out_the_lines(&mut self) {
        for at in 0..self.rides.len() {
            let ride = self.rides[at].id();
            let gone: Vec<u32> = self.rides[at]
                .queue()
                .waiting()
                .iter()
                .copied()
                .filter(|id| {
                    self.guests
                        .iter()
                        .find(|guest| guest.id() == *id)
                        .is_none_or(|guest| {
                            guest.queueing_for().map(|(ride, _)| ride) != Some(ride)
                        })
                })
                .collect();

            for id in gone {
                self.rides[at].queue_mut().leave(id);
            }
        }
    }

    /// Puts guests who have walked to a line into it.
    fn join_the_queues(&mut self) {
        for index in 0..self.guests.len() {
            let Plan::Queueing { ride, station, .. } = self.guests[index].plan() else {
                continue;
            };

            let Ok(at) = self.index_of_ride(ride) else {
                // The ride was taken down while somebody was walking to it.
                self.guests[index].decide(Plan::Wandering);
                continue;
            };

            let id = self.guests[index].id();
            if self.rides[at].queue().holds(id) {
                continue;
            }

            if !self.rides[at].is_open() {
                self.guests[index].decide(Plan::Wandering);
                continue;
            }

            // Only from a tile the line actually stands on: a queue is joined
            // at the back, not barged into from the side.
            let line = queue::line_from(&self.land, station, |tile| self.is_standing_room(tile));
            if !line.contains(&self.guests[index].tile()) {
                continue;
            }

            // A line holds as many as there is path to stand on. Laying more
            // queue is what makes a popular ride able to hold its crowd.
            if self.rides[at].queue().len() >= line.len() || !self.rides[at].queue_mut().join(id) {
                self.guests[index].decide(Plan::Wandering);
                continue;
            }

            // Whatever route it walked to get here is finished with: standing
            // still lets the shuffle put it in its place.
            self.guests[index].stop();
        }
    }

    /// Walks everybody in a line towards the place they are standing in it.
    ///
    /// A guest already on its place stands still. One that is not gets a route
    /// to it, which is what makes a queue shuffle forward a tile at a time as
    /// the front of it boards.
    fn shuffle_the_queues(&mut self) {
        for at in 0..self.rides.len() {
            let Some(station) = self.rides[at].stations().first().copied() else {
                continue;
            };

            let line = queue::line_from(&self.land, station, |tile| self.is_standing_room(tile));
            if line.is_empty() {
                continue;
            }

            let waiting: Vec<u32> = self.rides[at].queue().waiting().to_vec();
            for (place, id) in waiting.into_iter().enumerate() {
                // Past the end of the path: stand where the line runs out.
                let standing = line[place.min(line.len() - 1)];

                let Some(guest) = self.guests.iter_mut().find(|guest| guest.id() == id) else {
                    continue;
                };
                if guest.tile() == standing || !guest.is_idle() {
                    continue;
                }

                let map = ParkMap {
                    land: &self.land,
                    facilities: &self.facilities,
                    scenery: &self.scenery,
                };
                let mut finder = PathFinder::new();
                if let Some(route) = finder.find(&map, guest.tile(), standing) {
                    let _ = guest.follow(route.tiles().to_vec());
                }
            }
        }
    }

    /// Puts the front of each line onto the train that is loading.
    ///
    /// A guest pays as it boards and the park keeps the fare, which is why the
    /// ride's till and the bank move together.
    fn load_the_trains(&mut self) {
        let mut fares = 0;

        for at in 0..self.rides.len() {
            let Some(station) = self.rides[at].stations().first().copied() else {
                continue;
            };
            let line = queue::line_from(&self.land, station, |tile| self.is_standing_room(tile));
            let Some(front_tile) = line.first().copied() else {
                continue;
            };

            // One a tick: a train loads a queue, it does not inhale it.
            while self.rides[at].boarding().is_some() {
                let Some(id) = self.rides[at].queue().front() else {
                    break;
                };
                let Some(index) = self.guests.iter().position(|guest| guest.id() == id) else {
                    self.rides[at].queue_mut().leave(id);
                    continue;
                };

                // Still walking up the line.
                if self.guests[index].tile() != front_tile {
                    break;
                }

                let price = self.rides[at].price();
                if !self.guests[index].can_afford(price) {
                    self.rides[at].queue_mut().leave(id);
                    self.guests[index].decide(Plan::Wandering);
                    continue;
                }

                let Some(fare) = self.rides[at].board_one() else {
                    break;
                };
                self.rides[at].queue_mut().leave(id);
                fares += self.guests[index].pay(fare);
                self.guests[index].decide_anyway(Plan::Riding {
                    ride: self.rides[at].id(),
                });
                break;
            }
        }

        self.adjust_cash(fares);
    }

    /// Lets go of anybody who has stood in a line longer than it was worth.
    ///
    /// A queue that never moves is the most reliable way to empty a park, so a
    /// guest that gives up on one leaves in a worse mood than it joined in.
    fn give_up_waiting(&mut self) {
        let now = self.tick;
        let mut gave_up: Vec<(usize, u32)> = Vec::new();

        for (index, guest) in self.guests.iter().enumerate() {
            let Some((ride, since)) = guest.queueing_for() else {
                continue;
            };

            if now.get().saturating_sub(since.get()) > u64::from(Queue::PATIENCE) {
                gave_up.push((index, ride));
            }
        }

        for (index, ride) in gave_up {
            let id = self.guests[index].id();
            if let Ok(at) = self.index_of_ride(ride) {
                self.rides[at].queue_mut().leave(id);
            }

            self.guests[index].put_off(Self::GAVE_UP_QUEUEING);
            self.guests[index].decide(Plan::Wandering);
            tracing::debug!(guest = id, ride, "gave up waiting");
        }
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

    /// Puts everybody who was in a line back on their feet.
    fn send_the_line_away(&mut self, sent_away: &[u32]) {
        for guest in &mut self.guests {
            if sent_away.contains(&guest.id()) {
                guest.decide(Plan::Wandering);
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
                let sent_away = self.rides[at].break_down();
                tracing::info!(ride = %self.rides[at].name(), wear, "a ride broke down");
                self.send_the_line_away(&sent_away);
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
            scenery,
            staff,
            rng,
            ..
        } = self;

        let map = ParkMap {
            land,
            facilities,
            scenery,
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

    /// How many ticks pass between arrivals, given what people think of the
    /// park.
    ///
    /// A park nobody has heard anything good about fills slowly; one with rides
    /// worth queueing for and something to look at between them fills ten times
    /// faster. It is the only advertising there is.
    fn how_often_people_turn_up(&self) -> u64 {
        let spread = Self::SLOWEST_ARRIVALS - Self::FASTEST_ARRIVALS;
        let word_of_mouth = u64::from(self.reputation) * spread / u64::from(Rating::BEST);

        Self::SLOWEST_ARRIVALS.saturating_sub(word_of_mouth)
    }

    /// Lets one guest in, if one is due and there is room.
    fn admit_a_guest(&mut self) {
        if !self.tick.is_multiple_of(self.how_often_people_turn_up())
            || self.guests.len() >= Self::CAPACITY
        {
            return;
        }

        #[allow(clippy::cast_possible_truncation)]
        let shirt = self.rng.next_u32() as u8;
        let (least, most) = Self::SPENDING_MONEY;
        let money = Money::from(self.rng.range(least, most));
        let (timid, fearless) = Guest::NERVE;
        let nerve = timid + self.rng.next_f32() * (fearless - timid);
        let (least, most) = Guest::TOLERANCE;
        let tolerance = least + self.rng.next_f32() * (most - least);

        let guest = Guest::arriving(self.next_guest_id, self.entrance(), shirt, money)
            .with_nerve(nerve)
            .with_tolerance(tolerance);

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
                scenery,
                guests,
                rides,
                rng,
                ..
            } = self;

            let map = ParkMap {
                land,
                facilities,
                scenery,
            };

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

                    guest.advance(Self::speed_across(ground) * Self::pace_of(guest));
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

                let route =
                    match Self::what_next(guest, &map, rng, &mut finder, entrance, rides, now) {
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
                guest.advance(Self::speed_across(standing_on) * Self::pace_of(guest));
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
        now: Tick,
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
                    return Some((
                        Plan::Queueing {
                            ride,
                            station,
                            since: now,
                        },
                        route,
                    ));
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

    /// How much faster than a stroll this guest is moving.
    ///
    /// Only one thing changes it: somebody shuffling up a queue is not out for
    /// a walk.
    fn pace_of(guest: &Guest) -> f32 {
        if guest.queueing_for().is_some() {
            Self::QUEUE_SHUFFLE
        } else {
            1.0
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

    use crate::park::fixtures::{park_with_a_coaster, run};
    use crate::park::{Queue, Ride, Train};

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
                scenery: &park.scenery,
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
            scenery: &park.scenery,
        };
        assert!(
            finder.find(&map, below, ramp).is_none(),
            "somebody climbed two steps at once"
        );
    }
    /// The same coaster, with a queue path laid from its station.
    ///
    /// Returns the park, the ride, and the line the guests should end up
    /// standing along.
    fn park_with_a_queue() -> (Park, u32, Vec<TilePos>) {
        let (mut park, id) = park_with_a_coaster();
        let station = park.ride(id).unwrap().stations()[0];

        // A straight run of queue path leading away from the station, in
        // whichever direction has the most room: the obvious neighbour heads
        // into the middle of the ring and runs out of space after two tiles.
        let room = |park: &Park, (dx, dy): (i32, i32)| {
            let mut tile = station;
            let mut count = 0;
            while count < 5 {
                let next = tile.offset(dx, dy);
                if park.ride_at(next).is_some() || !park.land().contains(next) {
                    break;
                }
                count += 1;
                tile = next;
            }
            count
        };

        let (dx, dy) = [(0, -1), (1, 0), (0, 1), (-1, 0)]
            .into_iter()
            .max_by_key(|way| room(&park, *way))
            .expect("four ways to point a queue");

        let mut laid = Vec::new();
        let mut tile = station;
        for _ in 0..room(&park, (dx, dy)) {
            let next = tile.offset(dx, dy);
            park.lay(next, Terrain::Queue)
                .expect("queue path should lay");
            laid.push(next);
            tile = next;
        }
        assert!(
            laid.len() >= 3,
            "the test needs a queue to stand in: laid {laid:?} from {station:?}"
        );

        let line = queue::line_from(park.land(), station, |tile| park.is_standing_room(tile));
        (park, id, line)
    }

    #[test]
    fn the_line_stands_along_the_path_that_was_laid_for_it() {
        let (park, id, line) = park_with_a_queue();
        assert!(line.len() >= 3, "the queue path was not found");

        let park = run(park, 6_000);
        let waiting = park.ride(id).expect("the ride is there").queue();
        assert!(!waiting.is_empty(), "nobody ever queued");

        for (place, id) in waiting.waiting().iter().enumerate() {
            let guest = park
                .guests()
                .iter()
                .find(|guest| guest.id() == *id)
                .expect("somebody in the line left the park");

            // Either standing on their place in the line, or still walking to
            // it — never somewhere else entirely.
            let standing = line[place.min(line.len() - 1)];
            assert!(
                guest.tile() == standing || !guest.is_idle() || line.contains(&guest.tile()),
                "guest {} is queueing from {:?}, which is not the line",
                guest.id(),
                guest.tile()
            );
        }
    }

    #[test]
    fn a_queue_is_served_from_the_front() {
        let (park, id) = park_with_a_coaster();
        let park = run(park, 8_000);
        let ride = park.ride(id).expect("the ride is there");

        assert!(ride.riders() > 0, "nobody got on");
        assert!(
            ride.queue().len() <= Queue::MAX_LENGTH,
            "the line grew past its cap"
        );
    }

    #[test]
    fn a_ride_that_breaks_down_turns_its_line_loose() {
        let (mut park, id, _) = park_with_a_queue();
        for _ in 0..6_000 {
            park.tick_once();
        }
        assert!(
            !park.ride(id).unwrap().queue().is_empty(),
            "nobody was queueing to be turned loose"
        );

        let at = park
            .rides()
            .iter()
            .position(|ride| ride.id() == id)
            .unwrap();
        let sent_away = park.rides[at].break_down();
        park.send_the_line_away(&sent_away);

        assert!(park.ride(id).unwrap().queue().is_empty(), "the line stayed");

        // Guests still walking towards it were never in the line, so they find
        // out when they arrive: a few hundred ticks later nobody is queueing
        // for a ride that is not running.
        let park = run(park, 2_000);
        assert!(
            park.guests()
                .iter()
                .all(|guest| guest.queueing_for().is_none()),
            "somebody is still queueing for a ride that broke down"
        );
    }

    #[test]
    fn nobody_stands_in_a_line_for_ever() {
        let (mut park, id, _) = park_with_a_queue();

        // Shut the ride with a queue already forming, but leave it standing:
        // the line has nothing to wait for, and should thin out on patience
        // alone rather than on the ride telling it to go.
        for _ in 0..6_000 {
            park.tick_once();
        }
        let queued = park.ride(id).unwrap().queue().len();
        assert!(queued > 0, "nobody joined the line");

        let park = run(park, u64::from(Queue::PATIENCE) * 2);
        let still_waiting = park
            .guests()
            .iter()
            .filter_map(Guest::queueing_for)
            .filter(|(ride, since)| {
                *ride == id && park.tick().get() - since.get() > u64::from(Queue::PATIENCE)
            })
            .count();

        assert_eq!(still_waiting, 0, "somebody waited past all patience");
    }

    #[test]
    fn a_queue_survives_a_save_with_everybody_in_their_place() {
        let (park, id, _) = park_with_a_queue();
        let park = run(park, 6_000);
        let waiting = park.ride(id).unwrap().queue().waiting().to_vec();
        assert!(!waiting.is_empty(), "nobody queued");

        let json = serde_json::to_string(&park).unwrap();
        let loaded: Park = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.ride(id).unwrap().queue().waiting(), waiting);
    }

    #[test]
    fn a_full_line_fills_a_whole_train() {
        let (park, id, _) = park_with_a_queue();
        let mut park = park;

        let mut fullest = 0;
        for _ in 0..12_000 {
            park.tick_once();
            fullest = fullest.max(
                park.ride(id)
                    .and_then(|ride| ride.trains().iter().map(Train::riders).max())
                    .unwrap_or(0),
            );
        }

        assert_eq!(
            fullest,
            Ride::SEATS,
            "the best the queue managed was {fullest} of {} seats: the line \
             cannot keep up with the train",
            Ride::SEATS
        );
    }

    #[test]
    fn somebody_who_gives_up_on_the_day_gives_up_their_place_in_the_line() {
        let (mut park, id, _) = park_with_a_queue();
        for _ in 0..6_000 {
            park.tick_once();
        }

        let waiting = park.ride(id).unwrap().queue().waiting().to_vec();
        let leaving = *waiting.first().expect("nobody was queueing");

        // The front of the line decides it has had enough. Without the line
        // being cleared out, nobody behind it would ever board again.
        for guest in &mut park.guests {
            if guest.id() == leaving {
                guest.decide(Plan::GoingHome);
            }
        }
        park.tick_once();

        assert!(
            !park.ride(id).unwrap().queue().holds(leaving),
            "somebody on their way home is still holding up the queue"
        );
    }
}
