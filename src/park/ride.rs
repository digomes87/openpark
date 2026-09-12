//! A ride: a layout, the trains that run it, and the physics that move them.
//!
//! A train is a position along the track and a speed, and the only things that
//! change the speed are gravity, rolling resistance, the chain lift, the brakes
//! and the station. That is the whole model, and it is enough for the thing that
//! makes a coaster a coaster: a layout has to earn its speed from its own
//! height, so a train that is sent round a hill it cannot climb stalls — and a
//! ride whose test run stalls is not allowed to open.
//!
//! Everything here is deterministic: the same layout, the same trains and the
//! same number of ticks give the same lap, because a save that reloaded into a
//! different lap would be a save of nothing.

use anyhow::{Context, Result};
use isogrid::iso::TilePos;
use serde::{Deserialize, Serialize};

use crate::park::{Money, Track, TrackPiece};

/// How much speed a train gains each tick while dropping a step per tile.
///
/// Tiles per tick per tick. The number that decides how a park feels: too small
/// and a coaster never gets going, too large and a lift hill is pointless.
const PULL_OF_A_DROP: f32 = 0.0012;

/// What fraction of its speed a train loses to friction each tick.
///
/// This is the number that decides how far a train can coast, and it is easy to
/// get badly wrong: losing a share of the speed every tick means a train
/// released at `v` covers `v / ROLLING_RESISTANCE` tiles before it stops,
/// whatever the layout. At a twentieth it can cross a dozen tiles on a
/// dispatch, which is what a lap needs; at a thirtieth it could not leave its
/// own station.
///
/// Against [`PULL_OF_A_DROP`] it also settles a bottomless drop at about a
/// quarter of a tile a tick — ten tiles a second at the classic tick rate.
const ROLLING_RESISTANCE: f32 = 0.005;

/// Below this, a coasting train has stopped.
const STALLED: f32 = 0.005;

impl Train {
    /// How fast the chain lift pulls, whatever the train arrives at.
    pub const LIFT_SPEED: f32 = 0.05;

    /// How fast a train leaves the station.
    pub const DISPATCH_SPEED: f32 = 0.03;

    /// The fastest the brakes will let a train past.
    pub const BRAKE_SPEED: f32 = 0.05;

    /// The fastest a train crosses its own station.
    pub const STATION_SPEED: f32 = 0.04;

    /// The pace driven track keeps, whatever the train arrived at.
    pub const POWERED_SPEED: f32 = 0.06;

    /// The fastest anything is allowed to go, however steep the layout.
    ///
    /// A cap rather than physics: it keeps a pathological layout from
    /// integrating its way into a train that skips whole pieces in a tick.
    pub const TOP_SPEED: f32 = 0.4;
}

/// One train on a layout.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Train {
    /// Which piece of the layout it is on.
    at: usize,
    /// How far across that piece it is, from 0 to 1.
    progress: f32,
    /// How fast it is going, in tiles per tick.
    speed: f32,
    /// How many guests are aboard.
    riders: u32,
    /// Ticks left standing in the station.
    dwell: u32,
}

impl Train {
    /// A train standing at the beginning of a layout, empty.
    pub const fn waiting() -> Self {
        Self {
            at: 0,
            progress: 0.0,
            speed: 0.0,
            riders: 0,
            dwell: 0,
        }
    }

    /// Which piece it is on.
    pub const fn at(&self) -> usize {
        self.at
    }

    /// How far across that piece it is.
    pub const fn progress(&self) -> f32 {
        self.progress
    }

    /// How fast it is going, in tiles per tick.
    pub const fn speed(&self) -> f32 {
        self.speed
    }

    /// How many guests are aboard.
    pub const fn riders(&self) -> u32 {
        self.riders
    }

    /// Whether it is standing in a station with its doors open.
    pub const fn is_loading(&self) -> bool {
        self.dwell > 0
    }

    /// Puts `riders` aboard and holds the train for `dwell` ticks.
    pub const fn load(&mut self, riders: u32, dwell: u32) {
        self.riders = riders;
        self.dwell = dwell;
    }

    /// Empties the train out, returning how many got off.
    pub const fn unload(&mut self) -> u32 {
        let got_off = self.riders;
        self.riders = 0;
        got_off
    }

    /// Moves the train one tick along `track`.
    ///
    /// Returns whether it crossed the end of the layout and started another
    /// lap, which is the caller's cue to unload it.
    ///
    /// Speed comes first and movement second, so a train that stalls on a climb
    /// stops where it is rather than creeping one more tick.
    fn advance(&mut self, track: &Track) -> bool {
        let laid = track.segments();
        if laid.is_empty() {
            return false;
        }

        self.at %= laid.len();
        let here = laid[self.at];

        if self.dwell > 0 {
            self.dwell -= 1;
            self.speed = 0.0;
            return false;
        }

        self.speed = Self::speed_on(here.piece, self.speed, f32::from(here.exit - here.entry));
        if self.speed <= STALLED && !here.piece.is_powered() {
            self.speed = 0.0;
            return false;
        }

        self.progress += self.speed;
        let mut lapped = false;
        while self.progress >= 1.0 {
            self.progress -= 1.0;
            self.at += 1;
            if self.at >= laid.len() {
                self.at = 0;
                lapped = true;
            }
        }

        lapped
    }

    /// What a train doing `speed` on `piece` is doing a tick later.
    ///
    /// `climb` is how many steps the piece rises, negative for a drop.
    fn speed_on(piece: TrackPiece, speed: f32, climb: f32) -> f32 {
        let coasting = (speed - climb * PULL_OF_A_DROP) * (1.0 - ROLLING_RESISTANCE);

        let speed = match piece {
            // Under power: the chain does not care how the train arrived.
            TrackPiece::LiftHill => Self::LIFT_SPEED,
            TrackPiece::Powered => Self::POWERED_SPEED,
            TrackPiece::Brakes => coasting.min(Self::BRAKE_SPEED),
            TrackPiece::Station => coasting.clamp(Self::DISPATCH_SPEED, Self::STATION_SPEED),
            _ => coasting,
        };

        speed.clamp(0.0, Self::TOP_SPEED)
    }
}

/// What a ride turned out to be like, once its layout was run.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RideStats {
    /// How much fun it is, from 0 to 1.
    pub excitement: f32,
    /// How rough it is, from 0 to 1.
    pub intensity: f32,
    /// The fastest a train got, in tiles per tick.
    pub top_speed: f32,
    /// How many ticks a lap takes.
    pub lap: u32,
}

/// Why a layout could not be sent round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestFailure {
    /// The track does not come back to the station.
    NotACircuit,
    /// There is nowhere for guests to get on.
    NoStation,
    /// The train ran out of speed partway round.
    Stalled,
}

impl TestFailure {
    /// What to tell whoever pressed the button.
    pub const fn why(self) -> &'static str {
        match self {
            Self::NotACircuit => "the track does not come back to the station",
            Self::NoStation => "there is nowhere for guests to get on",
            Self::Stalled => "the test train stalled: it needs more height to start with",
        }
    }
}

/// Whether a ride is running, and why not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RideState {
    /// Still being laid: not tested, cannot open.
    #[default]
    Building,
    /// Tested and shut: everything works, nobody is being let on.
    Closed,
    /// Running.
    Open,
    /// Stopped until a mechanic gets to it.
    Broken,
}

/// One ride.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ride {
    id: u32,
    name: String,
    track: Track,
    trains: Vec<Train>,
    price: Money,
    state: RideState,
    stats: Option<RideStats>,
    /// How many guests have ridden it.
    riders: u32,
    /// What its till has taken.
    takings: Money,
    /// How worn it is, from 0 (new) to 1 (about to fail).
    wear: f32,
}

impl Ride {
    /// What a new ride charges until its owner says otherwise.
    pub const DEFAULT_PRICE: Money = 15;

    /// The most anything can charge for a ride.
    pub const MAX_PRICE: Money = 100;

    /// How much one nudge of the price tool moves it.
    pub const PRICE_STEP: Money = 2;

    /// How many guests one train carries.
    pub const SEATS: u32 = 8;

    /// How long a train stands in the station, loading.
    pub const DWELL: u32 = 120;

    /// How many ticks a test run is given to come back round.
    ///
    /// A lap that takes longer than this has either stalled somewhere the stall
    /// check cannot see or is too long to be worth waiting for.
    const TEST_PATIENCE: u32 = 20_000;

    /// How much more wear a rough ride takes than a gentle one.
    const ROUGHNESS_WEAR: f32 = 2.0;

    /// How much wear one lap adds to a gentle ride.
    const WEAR_PER_LAP: f32 = 0.004;

    /// A ride with a name and somewhere to start laying track.
    pub fn new(id: u32, name: impl Into<String>, track: Track) -> Self {
        Self {
            id,
            name: name.into(),
            track,
            trains: Vec::new(),
            price: Self::DEFAULT_PRICE,
            state: RideState::Building,
            stats: None,
            riders: 0,
            takings: 0,
            wear: 0.0,
        }
    }

    /// Which ride this is.
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// What it is called.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its layout.
    pub const fn track(&self) -> &Track {
        &self.track
    }

    /// Its layout, to lay more track on.
    ///
    /// Changing the track un-tests the ride: whatever it was measured at no
    /// longer describes what is on the ground.
    pub fn track_mut(&mut self) -> &mut Track {
        self.stats = None;
        self.state = RideState::Building;
        self.trains.clear();
        &mut self.track
    }

    /// The trains on it.
    pub fn trains(&self) -> &[Train] {
        &self.trains
    }

    /// What it charges.
    pub const fn price(&self) -> Money {
        self.price
    }

    /// Sets what it charges, clamped to something a park can actually ask.
    pub fn set_price(&mut self, price: Money) -> Money {
        self.price = price.clamp(0, Self::MAX_PRICE);
        self.price
    }

    /// Puts the price up by one step.
    pub fn raise_price(&mut self) -> Money {
        self.set_price(self.price.saturating_add(Self::PRICE_STEP))
    }

    /// Brings it down by one step.
    pub fn lower_price(&mut self) -> Money {
        self.set_price(self.price.saturating_sub(Self::PRICE_STEP))
    }

    /// Whether it is running.
    pub const fn state(&self) -> RideState {
        self.state
    }

    /// Whether guests can get on right now.
    pub fn is_open(&self) -> bool {
        self.state == RideState::Open
    }

    /// What it was measured at, once it has been tested.
    pub const fn stats(&self) -> Option<RideStats> {
        self.stats
    }

    /// How many guests have ridden it.
    pub const fn riders(&self) -> u32 {
        self.riders
    }

    /// What its till has taken.
    pub const fn takings(&self) -> Money {
        self.takings
    }

    /// How worn it is, from 0 to 1.
    pub const fn wear(&self) -> f32 {
        self.wear
    }

    /// What it costs to keep in service for one wage bill.
    pub fn upkeep(&self) -> Money {
        self.track.upkeep()
    }

    /// Sends a test train round the layout, and records what it found.
    ///
    /// # Errors
    ///
    /// Returns why the layout will not do: no station, not a circuit, or a
    /// train that ran out of speed on the way round.
    ///
    /// ```
    /// # use openpark::park::{Heading, Ride, Track, TrackPiece};
    /// # use isogrid::iso::TilePos;
    /// let mut track = Track::starting_at(TilePos::new(8, 8), Heading::North, 0);
    /// for piece in [
    ///     TrackPiece::CurveRight,
    ///     TrackPiece::Station,
    ///     TrackPiece::CurveRight,
    ///     TrackPiece::CurveRight,
    ///     TrackPiece::Straight,
    ///     TrackPiece::CurveRight,
    /// ] {
    ///     track.push(piece)?;
    /// }
    ///
    /// let mut ride = Ride::new(0, "The Flat Oval", track);
    /// let stats = ride.test()?;
    /// assert!(stats.lap > 0, "a lap takes time");
    /// assert!(ride.open().is_ok(), "a tested circuit can open");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn test(&mut self) -> Result<RideStats, TestFailure> {
        if self.track.stations().is_empty() {
            return Err(TestFailure::NoStation);
        }
        if !self.track.is_a_circuit() {
            return Err(TestFailure::NotACircuit);
        }

        let mut train = Train::waiting();
        train.speed = Train::DISPATCH_SPEED;

        let mut top_speed = train.speed;
        let mut lap = 0;
        while lap < Self::TEST_PATIENCE {
            lap += 1;
            let lapped = train.advance(&self.track);
            top_speed = top_speed.max(train.speed);

            if train.speed <= 0.0 {
                return Err(TestFailure::Stalled);
            }
            if lapped {
                let stats = self.measure(top_speed, lap);
                self.stats = Some(stats);
                self.state = RideState::Closed;
                return Ok(stats);
            }
        }

        Err(TestFailure::Stalled)
    }

    /// What the layout adds up to, given how a test run went.
    ///
    /// Excitement comes from the drop, the turns and the length of the thing;
    /// intensity from how fast it goes and how far it falls. A ride that is all
    /// intensity and no excitement is a fairground ride nobody queues twice
    /// for, which is what the subtraction at the end is about.
    fn measure(&self, top_speed: f32, lap: u32) -> RideStats {
        let drop = f32::from(self.track.longest_drop());
        #[allow(clippy::cast_precision_loss)]
        let turns = self.track.curves() as f32;
        #[allow(clippy::cast_precision_loss)]
        let length = self.track.len() as f32;

        let intensity = (top_speed / Train::TOP_SPEED * 0.6 + drop / 8.0 * 0.4).clamp(0.0, 1.0);
        let excitement = (0.15
            + drop / 8.0 * 0.35
            + turns / 8.0 * 0.2
            + length / 40.0 * 0.2
            + top_speed / Train::TOP_SPEED * 0.2
            - (intensity - 0.85).max(0.0) * 2.0)
            .clamp(0.0, 1.0);

        RideStats {
            excitement,
            intensity,
            top_speed,
            lap,
        }
    }

    /// Opens the ride, putting one train in the station.
    ///
    /// # Errors
    ///
    /// Fails if the ride has not been tested since its track last changed, or
    /// if it is broken down.
    pub fn open(&mut self) -> Result<()> {
        self.stats
            .context("a ride has to be tested before it can open")?;
        anyhow::ensure!(
            self.state != RideState::Broken,
            "{} is broken down and needs a mechanic",
            self.name,
        );

        if self.trains.is_empty() {
            let mut train = Train::waiting();
            train.load(0, Self::DWELL);
            self.trains.push(train);
        }

        self.state = RideState::Open;
        Ok(())
    }

    /// Shuts the ride without taking anything down.
    pub const fn close(&mut self) {
        self.state = RideState::Closed;
    }

    /// Breaks the ride down, stopping every train where it stands.
    pub fn break_down(&mut self) {
        self.state = RideState::Broken;
        for train in &mut self.trains {
            train.speed = 0.0;
        }
    }

    /// Puts a broken ride back together, as good as half new.
    ///
    /// A repair takes the wear back to the middle rather than to nothing: an old
    /// ride stays an old ride, and eventually wants replacing.
    pub fn repair(&mut self) {
        if self.state == RideState::Broken {
            self.state = RideState::Closed;
        }
        self.wear = (self.wear / 2.0).clamp(0.0, 1.0);
    }

    /// Whether a guest would be let on right now, and which station tile they
    /// would board at.
    pub fn boarding(&self) -> Option<TileBoarding> {
        if !self.is_open() {
            return None;
        }

        let seats_left = self
            .trains
            .iter()
            .filter(|train| train.is_loading())
            .map(|train| Self::SEATS.saturating_sub(train.riders()))
            .max()?;
        if seats_left == 0 {
            return None;
        }

        Some(TileBoarding {
            station: *self.track.stations().first()?,
            seats_left,
        })
    }

    /// Puts one more guest on the train that is loading.
    ///
    /// Returns what they were charged, or `None` if there was no room after
    /// all.
    pub fn board_one(&mut self) -> Option<Money> {
        let train = self
            .trains
            .iter_mut()
            .filter(|train| train.is_loading() && train.riders() < Self::SEATS)
            .max_by_key(|train| Self::SEATS - train.riders())?;

        train.riders += 1;
        self.riders = self.riders.saturating_add(1);
        self.takings = self.takings.saturating_add(self.price);
        Some(self.price)
    }

    /// Runs the ride for one tick.
    ///
    /// Returns how many guests got off at the station, which the park turns back
    /// into people standing next to it.
    pub fn tick(&mut self) -> u32 {
        if self.state != RideState::Open {
            return 0;
        }

        let intensity = self.stats.map_or(0.0, |stats| stats.intensity);
        let mut got_off = 0;

        for index in 0..self.trains.len() {
            let mut train = self.trains[index];
            let was_loading = train.is_loading();
            let lapped = train.advance(&self.track);

            if lapped {
                got_off += train.unload();
                train.load(0, Self::DWELL);
                self.wear = (self.wear
                    + Self::WEAR_PER_LAP * (1.0 + intensity * Self::ROUGHNESS_WEAR))
                    .clamp(0.0, 1.0);
            } else if was_loading && !train.is_loading() {
                // Just dispatched: the station gives it its push.
                train.speed = Train::DISPATCH_SPEED;
            }

            self.trains[index] = train;
        }

        got_off
    }
}

/// Where guests get on a ride, and how many can.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileBoarding {
    /// The station tile to queue beside.
    pub station: TilePos,
    /// How many seats are left on the loading train.
    pub seats_left: u32,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    use crate::park::Heading;
    use isogrid::iso::TilePos;

    /// A four-by-four ring: a curve on each corner and two pieces along each
    /// side, laid clockwise from the top-left corner.
    ///
    /// Every layout below is this shape with different pieces along the sides,
    /// which keeps the tests about the physics rather than about geometry —
    /// the shape closing is [`Track::is_a_circuit`]'s business, and tested
    /// there.
    fn ring(sides: [[TrackPiece; 2]; 4]) -> Track {
        let mut track = Track::starting_at(TilePos::new(4, 4), Heading::North, 0);
        for side in sides {
            track
                .push(TrackPiece::CurveRight)
                .expect("a corner should lay");
            for piece in side {
                track.push(piece).expect("a side should lay");
            }
        }

        assert!(track.is_a_circuit(), "the ring does not close");
        track
    }

    /// A gentle circuit: flat, driven, and open to anybody.
    fn powered_oval() -> Track {
        ring([
            [TrackPiece::Station, TrackPiece::Powered],
            [TrackPiece::Powered, TrackPiece::Powered],
            [TrackPiece::Powered, TrackPiece::Powered],
            [TrackPiece::Powered, TrackPiece::Powered],
        ])
    }

    /// A coaster: chained up three steps, dropped down three, and braked before
    /// the station.
    fn coaster() -> Track {
        ring([
            [TrackPiece::Station, TrackPiece::LiftHill],
            [TrackPiece::LiftHill, TrackPiece::LiftHill],
            [TrackPiece::SlopeDown, TrackPiece::SlopeDown],
            [TrackPiece::SlopeDown, TrackPiece::Brakes],
        ])
    }

    /// The same coaster with no chain on the hill: a train dispatched into it
    /// has nothing to climb with.
    fn unpowered_hill() -> Track {
        ring([
            [TrackPiece::Station, TrackPiece::SlopeUp],
            [TrackPiece::SlopeUp, TrackPiece::SlopeUp],
            [TrackPiece::SlopeDown, TrackPiece::SlopeDown],
            [TrackPiece::SlopeDown, TrackPiece::Straight],
        ])
    }

    #[test]
    fn a_ride_starts_unbuilt_and_unmeasured() {
        let ride = Ride::new(
            0,
            "The Nothing",
            Track::starting_at(TilePos::ORIGIN, Heading::North, 0),
        );

        assert_eq!(ride.state(), RideState::Building);
        assert_eq!(ride.stats(), None);
        assert!(!ride.is_open());
        assert_eq!(ride.price(), Ride::DEFAULT_PRICE);
        assert!(ride.trains().is_empty());
        assert_eq!(ride.boarding(), None);
    }

    #[test]
    fn a_layout_with_no_station_cannot_be_tested() {
        let mut track = Track::starting_at(TilePos::new(4, 4), Heading::East, 0);
        for _ in 0..4 {
            track.push(TrackPiece::CurveRight).unwrap();
        }
        assert!(track.is_a_circuit(), "four curves should close");

        let mut ride = Ride::new(0, "No Way On", track);
        assert_eq!(ride.test(), Err(TestFailure::NoStation));
    }

    #[test]
    fn a_layout_that_does_not_meet_up_cannot_be_tested() {
        let mut track = Track::starting_at(TilePos::new(4, 4), Heading::East, 0);
        track.push(TrackPiece::Station).unwrap();
        track.push(TrackPiece::Straight).unwrap();

        let mut ride = Ride::new(0, "The Dead End", track);
        assert_eq!(ride.test(), Err(TestFailure::NotACircuit));
        assert!(ride.open().is_err(), "an untested ride opened");
    }

    #[test]
    fn a_train_cannot_climb_a_hill_it_has_no_speed_for() {
        let mut ride = Ride::new(0, "The Wall", unpowered_hill());

        assert_eq!(ride.test(), Err(TestFailure::Stalled));
        assert!(ride.open().is_err(), "a stalling ride opened");
    }

    #[test]
    fn a_flat_circuit_needs_driving_and_a_coaster_does_not() {
        let mut driven = Ride::new(0, "The Monorail", powered_oval());
        assert!(
            driven.test().is_ok(),
            "driven track could not keep a flat lap going"
        );

        // The same shape with plain track under it runs out of speed, which is
        // why powered track exists.
        let mut coasting = Ride::new(
            1,
            "The Flat Oval",
            ring([
                [TrackPiece::Station, TrackPiece::Straight],
                [TrackPiece::Straight, TrackPiece::Straight],
                [TrackPiece::Straight, TrackPiece::Straight],
                [TrackPiece::Straight, TrackPiece::Straight],
            ]),
        );
        assert_eq!(coasting.test(), Err(TestFailure::Stalled));
    }

    #[test]
    fn a_lift_hill_pulls_a_train_up_whatever_its_speed() {
        let mut ride = Ride::new(0, "The Coaster", coaster());
        let stats = ride.test().expect("a lift hill should get it round");

        assert!(stats.lap > 0);
        assert!(
            stats.top_speed > Train::LIFT_SPEED,
            "the drops added nothing: top speed was {}",
            stats.top_speed
        );
        assert!(stats.top_speed <= Train::TOP_SPEED);
    }

    #[test]
    fn a_drop_is_where_the_speed_comes_from() {
        let downhill = Train::speed_on(TrackPiece::SlopeDown, 0.05, -1.0);
        let flat = Train::speed_on(TrackPiece::Straight, 0.05, 0.0);
        let uphill = Train::speed_on(TrackPiece::SlopeUp, 0.05, 1.0);

        assert!(downhill > flat, "falling did not help");
        assert!(flat < 0.05, "friction did nothing on the flat");
        assert!(uphill < flat, "climbing was free");
    }

    #[test]
    fn friction_settles_a_long_descent_at_a_sane_speed() {
        let mut speed = 0.05;
        for _ in 0..2_000 {
            speed = Train::speed_on(TrackPiece::SlopeDown, speed, -1.0);
        }

        assert!(
            (0.1..=Train::TOP_SPEED).contains(&speed),
            "a bottomless drop settled at {speed}"
        );
    }

    #[test]
    fn the_brakes_and_the_station_hold_a_train_back() {
        let fast = Train::TOP_SPEED;
        assert!(Train::speed_on(TrackPiece::Brakes, fast, 0.0) <= Train::BRAKE_SPEED);
        assert!(Train::speed_on(TrackPiece::Station, fast, 0.0) <= Train::STATION_SPEED);
        assert_eq!(
            Train::speed_on(TrackPiece::LiftHill, 0.0, 1.0),
            Train::LIFT_SPEED,
            "the chain does not care how slowly it arrived"
        );
    }

    #[test]
    fn a_rougher_ride_is_more_intense_than_a_gentle_one() {
        let mut gentle = Ride::new(0, "The Monorail", powered_oval());
        let mut rough = Ride::new(1, "The Coaster", coaster());

        let gentle = gentle.test().expect("the oval should run");
        let rough = rough.test().expect("the coaster should run");

        assert!(
            rough.intensity > gentle.intensity,
            "a coaster with three drops is no rougher than a driven flat oval"
        );
        assert!(
            rough.excitement > gentle.excitement,
            "and no more fun either"
        );
    }

    #[test]
    fn a_tested_ride_opens_with_a_train_in_the_station() {
        let mut ride = Ride::new(0, "The Coaster", coaster());
        ride.test().unwrap();
        ride.open().unwrap();

        assert!(ride.is_open());
        assert_eq!(ride.trains().len(), 1);
        assert!(ride.trains()[0].is_loading(), "it should be loading");

        let boarding = ride.boarding().expect("guests should be let on");
        assert_eq!(boarding.seats_left, Ride::SEATS);
        assert!(ride.track().stations().contains(&boarding.station));
    }

    #[test]
    fn laying_more_track_un_tests_a_ride() {
        let mut ride = Ride::new(0, "The Coaster", coaster());
        ride.test().unwrap();
        ride.open().unwrap();

        ride.track_mut().pop();
        assert_eq!(ride.state(), RideState::Building);
        assert_eq!(ride.stats(), None);
        assert!(ride.trains().is_empty());
        assert!(ride.open().is_err());
    }

    #[test]
    fn guests_pay_as_they_board_and_are_counted_when_they_do() {
        let mut ride = Ride::new(0, "The Coaster", coaster());
        ride.test().unwrap();
        ride.open().unwrap();
        ride.set_price(20);

        for expected in 1..=Ride::SEATS {
            assert_eq!(ride.board_one(), Some(20));
            assert_eq!(ride.riders(), expected);
        }

        assert_eq!(
            ride.board_one(),
            None,
            "a ninth guest got on an eight seater"
        );
        assert_eq!(ride.takings(), 20 * i64::from(Ride::SEATS));
        assert_eq!(ride.boarding(), None, "a full train is still boarding");
    }

    #[test]
    fn a_train_goes_round_and_brings_everybody_back() {
        let mut ride = Ride::new(0, "The Coaster", coaster());
        let stats = ride.test().unwrap();
        ride.open().unwrap();
        ride.board_one().unwrap();

        let mut got_off = 0;
        for _ in 0..(stats.lap + Ride::DWELL * 2) {
            got_off += ride.tick();
        }

        assert_eq!(got_off, 1, "the rider never got off");
        assert!(ride.wear() > 0.0, "a lap wore nothing out");
    }

    #[test]
    fn a_shut_ride_does_not_move() {
        let mut ride = Ride::new(0, "The Coaster", coaster());
        ride.test().unwrap();
        ride.open().unwrap();
        ride.close();

        let before = ride.trains()[0];
        assert_eq!(ride.tick(), 0);
        assert_eq!(ride.trains()[0], before, "a shut ride moved");
        assert_eq!(ride.boarding(), None);
    }

    #[test]
    fn a_broken_ride_stops_and_cannot_open_until_it_is_fixed() {
        let mut ride = Ride::new(0, "The Coaster", coaster());
        ride.test().unwrap();
        ride.open().unwrap();
        for _ in 0..Ride::DWELL * 2 {
            ride.tick();
        }

        ride.break_down();
        assert_eq!(ride.state(), RideState::Broken);
        assert!(ride.trains().iter().all(|train| train.speed() == 0.0));
        assert!(ride.open().is_err(), "a broken ride opened");

        ride.repair();
        assert_eq!(ride.state(), RideState::Closed);
        assert!(ride.open().is_ok());
    }

    #[test]
    fn a_repair_leaves_an_old_ride_old() {
        let mut ride = Ride::new(0, "The Coaster", coaster());
        ride.test().unwrap();
        ride.open().unwrap();
        for _ in 0..20_000 {
            ride.tick();
        }

        let worn = ride.wear();
        assert!(worn > 0.0, "nothing wore out in twenty thousand ticks");

        ride.repair();
        assert!(ride.wear() < worn, "the repair did nothing");
        assert!(ride.wear() > 0.0, "the repair made it new again");
    }

    #[test]
    fn a_price_never_leaves_its_range() {
        let mut ride = Ride::new(0, "The Coaster", coaster());
        assert_eq!(ride.set_price(-10), 0);
        assert_eq!(ride.set_price(10_000), Ride::MAX_PRICE);

        ride.set_price(10);
        assert_eq!(ride.raise_price(), 10 + Ride::PRICE_STEP);
        assert_eq!(ride.lower_price(), 10);
    }

    #[test]
    fn a_ride_survives_a_save_mid_lap() {
        let mut ride = Ride::new(3, "The Coaster", coaster());
        ride.test().unwrap();
        ride.open().unwrap();
        ride.board_one().unwrap();
        for _ in 0..Ride::DWELL + 50 {
            ride.tick();
        }

        let json = serde_json::to_string(&ride).unwrap();
        let loaded: Ride = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded, ride);
        assert_eq!(loaded.trains()[0].at(), ride.trains()[0].at());
        assert_eq!(loaded.stats(), ride.stats());
    }

    #[test]
    fn the_same_layout_runs_the_same_lap_every_time() {
        let mut one = Ride::new(0, "The Coaster", coaster());
        let mut two = Ride::new(0, "The Coaster", coaster());

        let first = one.test().unwrap();
        let second = two.test().unwrap();
        assert_eq!(first, second, "the same layout gave two different rides");

        one.open().unwrap();
        two.open().unwrap();
        for _ in 0..3_000 {
            one.tick();
            two.tick();
        }
        assert_eq!(one.trains(), two.trains());
    }
}
