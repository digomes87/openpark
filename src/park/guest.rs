//! The people a park is built for.
//!
//! A guest is a position and a route, nothing more yet. It knows how to walk
//! the route it was given; deciding where that route goes is the park's job,
//! which keeps the walking testable without a map.

use anyhow::Context;
use isogrid::iso::{GridPoint, TilePos};
use isogrid::render::Color;
use isogrid::time::Tick;
use serde::{Deserialize, Serialize};

use crate::park::{Facility, Money, Needs, Ride, Shop, Walk};

/// The shirts guests turn up in.
///
/// Placeholder art, like the terrain: flat colours picked to stay legible
/// against grass, path and water alike.
const SHIRTS: [Color; 6] = [
    Color::hex(0xD9_54_4D),
    Color::hex(0xE8_9C_2E),
    Color::hex(0xE8_D6_4A),
    Color::hex(0x4D_9D_D9),
    Color::hex(0x9B_5D_C4),
    Color::hex(0xE8_E4_DC),
];

/// What a guest is trying to do with the rest of its route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Plan {
    /// Having a look around, with nothing particular in mind.
    Wandering,
    /// On the way to the facility standing on `facility`, which the guest will
    /// use from the tile beside it.
    Visiting { facility: TilePos },
    /// Standing at `facility`, busy until `until`.
    Using { facility: TilePos, until: Tick },
    /// Waiting for `ride`, in the line that leads to `station`, since `since`.
    Queueing {
        ride: u32,
        station: TilePos,
        since: Tick,
    },
    /// Aboard `ride`, and out of the park's way until it comes back round.
    Riding { ride: u32 },
    /// On the way to the gate, and out of the park once it gets there.
    GoingHome,
}

impl Plan {
    /// The facility this plan is about, if it is about one.
    pub const fn facility(self) -> Option<TilePos> {
        match self {
            Self::Visiting { facility } | Self::Using { facility, .. } => Some(facility),
            Self::Wandering | Self::GoingHome | Self::Queueing { .. } | Self::Riding { .. } => None,
        }
    }

    /// The ride this plan is about, if it is about one.
    pub const fn ride(self) -> Option<u32> {
        match self {
            Self::Queueing { ride, .. } | Self::Riding { ride } => Some(ride),
            Self::Wandering | Self::Visiting { .. } | Self::Using { .. } | Self::GoingHome => None,
        }
    }
}

/// One visitor.
///
/// Guests move between tile centres at a speed the park chooses, so a guest is
/// always somewhere on the segment between the tile it is standing on and the
/// next tile of its route.
///
/// ```
/// # use openpark::park::Guest;
/// # use isogrid::iso::TilePos;
/// let gate = TilePos::new(4, 0);
/// let mut guest = Guest::arriving(1, gate, 0, 100);
/// assert!(guest.is_idle(), "a guest with nowhere to go stands still");
///
/// guest.follow(vec![gate, gate.offset(0, 1)])?;
/// guest.advance(0.5);
/// assert_eq!(guest.tile(), gate, "still half a tile from the next one");
/// assert!(guest.position().y > gate.centre().y, "but on its way");
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guest {
    id: u32,
    /// Where it is and where it is going.
    walk: Walk,
    /// Which of [`SHIRTS`] this guest wears.
    shirt: u8,
    /// How the visit is going.
    needs: Needs,
    /// What the guest is doing about it.
    plan: Plan,
    /// What is left in its pocket.
    money: Money,
    /// How much over the fair price this guest will stand, as a multiple of it.
    ///
    /// Drawn when the guest arrives, so a park full of people disagrees about
    /// whether a stall is a rip-off rather than emptying all at once.
    tolerance: f32,
}

impl Guest {
    /// How much mood a guest loses on paying more than it thinks is fair.
    const OVERCHARGED: f32 = 0.08;

    /// What a guest will pay for something over the odds without doing the
    /// arithmetic, whatever the thing normally costs.
    const ODD_COINS: Money = 3;

    /// What a guest would pay for the most exciting ride imaginable.
    ///
    /// Everything else is a fraction of it, by excitement — so the way to
    /// charge more is to build something better rather than to put the price up.
    const WORTH_OF_A_THRILL: Money = 40;

    /// The least and the most a guest will pay over the fair price, as a
    /// multiple of it.
    ///
    /// Somebody who will not pay a penny over the odds, somebody who will pay
    /// half again, and everybody in between.
    pub const TOLERANCE: (f32, f32) = (1.0, 1.5);

    /// A guest who has just walked through the gate at `at`, carrying `money`
    /// to spend inside.
    ///
    /// Arrives with the average tolerance for a price. Use
    /// [`Guest::with_tolerance`] to give it an opinion of its own.
    pub fn arriving(id: u32, at: TilePos, shirt: u8, money: Money) -> Self {
        let (least, most) = Self::TOLERANCE;
        Self {
            id,
            walk: Walk::standing_at(at),
            shirt,
            needs: Needs::fresh(),
            plan: Plan::Wandering,
            money,
            tolerance: f32::midpoint(least, most),
        }
    }

    /// The same guest, but harder or easier to sell to.
    ///
    /// Clamped to [`Guest::TOLERANCE`]: a guest that would pay ten times the
    /// going rate is not a guest, it is a bug in the dice.
    #[must_use]
    pub fn with_tolerance(mut self, tolerance: f32) -> Self {
        let (least, most) = Self::TOLERANCE;
        self.tolerance = if tolerance.is_nan() {
            least
        } else {
            tolerance.clamp(least, most)
        };
        self
    }

    /// How much over the fair price this guest will stand.
    pub const fn tolerance(&self) -> f32 {
        self.tolerance
    }

    /// Which guest this is. Unique within one park, and stable across a save.
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// The tile the guest is standing on, or walking away from.
    pub fn tile(&self) -> TilePos {
        self.walk.tile()
    }

    /// The tile the guest is walking towards, if it is walking anywhere.
    pub fn next_tile(&self) -> Option<TilePos> {
        self.walk.next_tile()
    }

    /// Whether the guest has run out of route and wants a new one.
    pub fn is_idle(&self) -> bool {
        self.walk.is_idle()
    }

    /// How far between its two tiles the guest is, from 0 to 1.
    pub const fn progress(&self) -> f32 {
        self.walk.progress()
    }

    /// Where the guest is, in grid space, between the two tiles of its step.
    pub fn position(&self) -> GridPoint {
        self.walk.position()
    }

    /// The colour this guest draws as.
    pub fn shirt_colour(&self) -> Color {
        SHIRTS[self.shirt as usize % SHIRTS.len()]
    }

    /// How the guest is feeling.
    pub const fn needs(&self) -> Needs {
        self.needs
    }

    /// What the guest is trying to do.
    pub const fn plan(&self) -> Plan {
        self.plan
    }

    /// The ride the guest is queueing for, and how long it has been waiting.
    pub const fn queueing_for(&self) -> Option<(u32, Tick)> {
        match self.plan {
            Plan::Queueing { ride, since, .. } => Some((ride, since)),
            _ => None,
        }
    }

    /// Whether the guest is on its way out.
    pub fn is_going_home(&self) -> bool {
        self.plan == Plan::GoingHome
    }

    /// The ride the guest is aboard, if it is aboard one.
    ///
    /// A guest on a ride is not on the map: it does not walk, it is not drawn,
    /// and nothing can be built under it until it gets off.
    pub const fn riding(&self) -> Option<u32> {
        match self.plan {
            Plan::Riding { ride } => Some(ride),
            _ => None,
        }
    }

    /// Puts the guest back on its feet beside the station.
    ///
    /// Used both when a ride brings it back and when the ride it was on is
    /// demolished underneath it.
    pub fn get_off(&mut self) {
        if self.riding().is_some() {
            self.plan = Plan::Wandering;
        }
    }

    /// Takes `price` off the guest, if it can pay.
    ///
    /// Returns what was actually handed over, which is nothing at all when the
    /// guest cannot afford it.
    pub fn pay(&mut self, price: Money) -> Money {
        if !self.can_afford(price) {
            return 0;
        }

        self.money -= price;
        price
    }

    /// Rides something as exciting and as rough as `excitement` and
    /// `intensity`.
    pub fn enjoy_a_ride(&mut self, excitement: f32, intensity: f32) {
        self.needs.enjoy_a_ride(excitement, intensity);
    }

    /// Changes the guest's mind.
    ///
    /// A guest that has decided to go home stays decided: a park with nothing
    /// in it cannot talk anyone into staying, and letting it would leave guests
    /// dithering at the gate forever.
    ///
    /// ```
    /// # use openpark::park::{Guest, Plan};
    /// # use isogrid::iso::TilePos;
    /// let mut guest = Guest::arriving(1, TilePos::ORIGIN, 0, 100);
    /// guest.decide(Plan::GoingHome);
    /// guest.decide(Plan::Wandering);
    /// assert_eq!(guest.plan(), Plan::GoingHome);
    /// ```
    pub fn decide(&mut self, plan: Plan) {
        if self.plan != Plan::GoingHome {
            self.plan = plan;
        }
    }

    /// Changes the guest's mind even if it had decided to go home.
    ///
    /// For the park putting somebody on a ride or taking them off one, which
    /// happens to whoever is standing there regardless of what they had
    /// planned.
    pub fn decide_anyway(&mut self, plan: Plan) {
        self.plan = plan;
    }

    /// What the guest has left to spend.
    pub const fn money(&self) -> Money {
        self.money
    }

    /// Whether the guest can pay `price` and still have it be worth asking.
    pub const fn can_afford(&self, price: Money) -> bool {
        self.money >= price
    }

    /// Uses a facility: pays for it, and gets what it came for.
    ///
    /// Returns what the guest handed over, which is the park's to keep. A guest
    /// that cannot afford the price pays nothing and gets nothing, which is the
    /// caller's mistake — [`Guest::can_afford`] is there to be asked first.
    ///
    /// ```
    /// # use openpark::park::{Facility, Guest, Shop};
    /// # use isogrid::iso::TilePos;
    /// let mut guest = Guest::arriving(1, TilePos::ORIGIN, 0, 100);
    /// let shop = Shop::new(Facility::FoodStall);
    /// let paid = guest.enjoy(&shop);
    ///
    /// assert_eq!(paid, shop.price());
    /// assert_eq!(guest.money(), 100 - paid);
    /// assert!(!guest.needs().wants_food());
    /// ```
    pub fn enjoy(&mut self, shop: &Shop) -> Money {
        let price = shop.price();
        if !self.can_afford(price) {
            return 0;
        }

        self.money -= price;
        match shop.kind() {
            Facility::FoodStall => self.needs.eat(),
            Facility::Bench => self.needs.rest(),
        }

        // Paid, used, and resentful about it: the guest got what it came for
        // and still thinks it was robbed.
        if price > self.most_it_would_pay(shop.kind()) {
            self.needs.dislike(Self::OVERCHARGED);
        }
        price
    }

    /// The most this guest would hand over for something of `kind`.
    ///
    /// A multiple of the fair price, or the fair price plus the loose change in
    /// its pocket, whichever is kinder — otherwise nobody would ever pay for
    /// something that is normally free, and a bench could never be a business.
    fn most_it_would_pay(&self, kind: Facility) -> Money {
        let fair = kind.price();
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
        let by_tolerance = (fair as f32 * self.tolerance) as Money;
        by_tolerance.max(fair.saturating_add(Self::ODD_COINS))
    }

    /// Whether this guest would walk over to `shop` and pay what it asks.
    ///
    /// Two separate refusals: what is in its pocket, and what it thinks the
    /// thing is worth. A free bench passes both, always.
    ///
    /// ```
    /// # use openpark::park::{Facility, Guest, Shop};
    /// # use isogrid::iso::TilePos;
    /// let guest = Guest::arriving(1, TilePos::ORIGIN, 0, 100);
    /// let mut shop = Shop::new(Facility::FoodStall);
    /// assert!(guest.will_pay(&shop), "the going rate is fine");
    ///
    /// shop.set_price(Shop::MAX_PRICE);
    /// assert!(!guest.will_pay(&shop), "that is a rip-off");
    /// ```
    pub fn will_pay(&self, shop: &Shop) -> bool {
        self.can_afford(shop.price()) && shop.price() <= self.most_it_would_pay(shop.kind())
    }

    /// Whether this guest would queue for `ride` at what it charges.
    ///
    /// What a ride is worth is what it is like: a guest will pay a good deal for
    /// something exciting and almost nothing for a circle of flat track, which
    /// is the whole reason to build something worth riding.
    ///
    /// ```
    /// # use openpark::park::{Guest, Heading, Ride, Track, TrackPiece};
    /// # use isogrid::iso::TilePos;
    /// # let mut track = Track::starting_at(TilePos::new(4, 4), Heading::North, 0);
    /// # for side in [
    /// #     [TrackPiece::Station, TrackPiece::LiftHill],
    /// #     [TrackPiece::LiftHill, TrackPiece::LiftHill],
    /// #     [TrackPiece::SlopeDown, TrackPiece::SlopeDown],
    /// #     [TrackPiece::SlopeDown, TrackPiece::Brakes],
    /// # ] {
    /// #     track.push(TrackPiece::CurveRight)?;
    /// #     for piece in side { track.push(piece)?; }
    /// # }
    /// let mut ride = Ride::new(0, "The Coaster", track);
    /// ride.test()?;
    ///
    /// let guest = Guest::arriving(1, TilePos::ORIGIN, 0, 200);
    /// ride.set_price(5);
    /// assert!(guest.will_ride(&ride), "a fiver for a coaster is a bargain");
    ///
    /// ride.set_price(Ride::MAX_PRICE);
    /// assert!(!guest.will_ride(&ride), "nobody pays that");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn will_ride(&self, ride: &Ride) -> bool {
        let Some(stats) = ride.stats() else {
            return false;
        };

        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
        let worth = (stats.excitement * Self::WORTH_OF_A_THRILL as f32 * self.tolerance) as Money;

        self.can_afford(ride.price()) && ride.price() <= worth.max(Self::ODD_COINS)
    }

    /// Something about the park spoils the visit a little.
    pub fn put_off(&mut self, amount: f32) {
        self.needs.dislike(amount);
    }

    /// Something about the park is a treat.
    pub fn cheer_up(&mut self, amount: f32) {
        self.needs.enjoy_yourself(amount);
    }

    /// One tick of being in the park: the guest gets hungrier and more tired,
    /// and its mood follows.
    ///
    /// Walking is harder work than standing about, so a guest that has run out
    /// of route wears down more slowly until it is given a new one.
    pub fn live(&mut self) {
        self.needs.wear_down(!self.is_idle());
    }

    /// Sends the guest off along a new route.
    ///
    /// The route must start where the guest is standing — a guest cannot
    /// teleport to the beginning of a path someone else walked.
    ///
    /// # Errors
    ///
    /// Fails if the route is empty or does not start at [`Guest::tile`].
    pub fn follow(&mut self, route: Vec<TilePos>) -> anyhow::Result<()> {
        self.walk
            .follow(route)
            .with_context(|| format!("guest {} cannot follow that route", self.id))
    }

    /// Walks `distance` tiles along the route, stopping at the end of it.
    pub fn advance(&mut self, distance: f32) {
        self.walk.advance(distance);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walking() -> Guest {
        let mut guest = Guest::arriving(1, TilePos::new(0, 0), 0, 100);
        guest
            .follow(vec![
                TilePos::new(0, 0),
                TilePos::new(0, 1),
                TilePos::new(0, 2),
            ])
            .unwrap();
        guest
    }

    #[test]
    fn a_new_guest_stands_where_it_arrived_with_nowhere_to_go() {
        let guest = Guest::arriving(7, TilePos::new(4, 0), 2, 100);
        assert_eq!(guest.id(), 7);
        assert_eq!(guest.tile(), TilePos::new(4, 0));
        assert!(guest.is_idle());
        assert_eq!(guest.position(), TilePos::new(4, 0).centre());
    }

    #[test]
    fn walking_moves_between_the_tiles_of_the_route() {
        let mut guest = walking();
        guest.advance(0.5);

        assert_eq!(guest.tile(), TilePos::new(0, 0));
        assert_eq!(guest.next_tile(), Some(TilePos::new(0, 1)));

        let half_way = guest.position();
        let (from, to) = (TilePos::new(0, 0).centre(), TilePos::new(0, 1).centre());
        assert!((half_way.y - f32::midpoint(from.y, to.y)).abs() < 1e-5);
        assert!((half_way.x - from.x).abs() < 1e-5);
    }

    #[test]
    fn a_long_stride_crosses_several_tiles_at_once() {
        let mut guest = walking();
        guest.advance(1.5);
        assert_eq!(guest.tile(), TilePos::new(0, 1));
        assert_eq!(guest.next_tile(), Some(TilePos::new(0, 2)));
    }

    #[test]
    fn a_guest_stops_at_the_end_of_its_route_however_hard_it_is_pushed() {
        let mut guest = walking();
        guest.advance(1_000.0);
        assert_eq!(guest.tile(), TilePos::new(0, 2));
        assert!(guest.is_idle());
        assert_eq!(guest.position(), TilePos::new(0, 2).centre());
    }

    #[test]
    fn a_nonsense_speed_leaves_the_guest_where_it_was() {
        let mut guest = walking();
        let before = guest.position();

        guest.advance(-1.0);
        guest.advance(f32::NAN);
        assert_eq!(guest.position(), before);
    }

    #[test]
    fn a_route_must_start_where_the_guest_is() {
        let mut guest = Guest::arriving(1, TilePos::new(0, 0), 0, 100);
        assert!(guest.follow(vec![]).is_err());
        assert!(guest.follow(vec![TilePos::new(5, 5)]).is_err());
        assert!(guest.follow(vec![TilePos::new(0, 0)]).is_ok());
    }

    #[test]
    fn a_new_route_starts_from_the_beginning() {
        let mut guest = walking();
        guest.advance(1.5);

        let here = guest.tile();
        guest.follow(vec![here, here.offset(1, 0)]).unwrap();
        assert_eq!(guest.tile(), here);
        assert_eq!(guest.position(), here.centre());
    }

    #[test]
    fn guests_wear_one_of_the_shirts_however_high_the_number() {
        for shirt in 0..=u8::MAX {
            let guest = Guest::arriving(0, TilePos::ORIGIN, shirt, 100);
            assert!(SHIRTS.contains(&guest.shirt_colour()));
        }
    }

    #[test]
    fn living_wears_a_guest_down_faster_while_it_walks() {
        let mut walker = walking();
        let mut loiterer = Guest::arriving(2, TilePos::ORIGIN, 0, 100);
        for _ in 0..100 {
            walker.live();
            loiterer.live();
        }

        assert!(walker.needs().hunger() > loiterer.needs().hunger());
        assert!(walker.needs().energy() < loiterer.needs().energy());
    }

    #[test]
    fn a_guest_arrives_meaning_to_look_around() {
        let guest = Guest::arriving(1, TilePos::ORIGIN, 0, 100);
        assert_eq!(guest.plan(), Plan::Wandering);
        assert!(!guest.is_going_home());
    }

    #[test]
    fn a_guest_who_has_decided_to_leave_cannot_be_talked_out_of_it() {
        let mut guest = Guest::arriving(1, TilePos::ORIGIN, 0, 100);
        guest.decide(Plan::GoingHome);
        assert!(guest.is_going_home());

        guest.decide(Plan::Wandering);
        assert!(guest.is_going_home());
    }

    #[test]
    fn a_guest_pays_for_what_it_uses() {
        let mut guest = Guest::arriving(1, TilePos::ORIGIN, 0, 100);
        // Standing about is restful, so this takes longer than a walk would.
        for _ in 0..Needs::TICKS_UNTIL_HUNGRY * 2 {
            guest.live();
        }
        assert!(guest.needs().wants_food());

        let paid = guest.enjoy(&Shop::new(Facility::FoodStall));
        assert_eq!(paid, Facility::FoodStall.price());
        assert_eq!(guest.money(), 100 - paid);
        assert!(!guest.needs().wants_food());
    }

    #[test]
    fn sitting_down_costs_nothing() {
        let mut guest = Guest::arriving(1, TilePos::ORIGIN, 0, 100);
        assert_eq!(guest.enjoy(&Shop::new(Facility::Bench)), 0);
        assert_eq!(guest.money(), 100);
    }

    #[test]
    fn a_guest_with_empty_pockets_gets_nothing() {
        let mut guest = Guest::arriving(1, TilePos::ORIGIN, 0, 0);
        for _ in 0..Needs::TICKS_UNTIL_HUNGRY * 2 {
            guest.live();
        }

        assert!(!guest.can_afford(Facility::FoodStall.price()));
        assert_eq!(guest.enjoy(&Shop::new(Facility::FoodStall)), 0);
        assert!(guest.needs().wants_food(), "it was fed for free");
        assert_eq!(guest.money(), 0);
    }

    #[test]
    fn a_plan_knows_which_facility_it_is_about() {
        let tile = TilePos::new(3, 4);
        assert_eq!(Plan::Wandering.facility(), None);
        assert_eq!(Plan::GoingHome.facility(), None);
        assert_eq!(Plan::Visiting { facility: tile }.facility(), Some(tile));
        assert_eq!(
            Plan::Using {
                facility: tile,
                until: Tick::new(10)
            }
            .facility(),
            Some(tile)
        );
    }

    #[test]
    fn a_guest_survives_a_save() {
        let guest = walking();
        let json = serde_json::to_string(&guest).unwrap();
        assert_eq!(serde_json::from_str::<Guest>(&json).unwrap(), guest);
    }
}
