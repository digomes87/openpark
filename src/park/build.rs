//! Changing the park: what it costs, and why it is refused.
//!
//! Everything a player does to a park with the mouse goes through here — putting
//! a stall up, pricing it, taking it down, moving the land, and laying track —
//! because it all has the same shape: check the money, check the ground, and
//! only then change anything. The checks are the interesting part, and keeping
//! them together is what stops them drifting apart.

use anyhow::{Context, Result};
use isogrid::iso::TilePos;

use crate::park::{
    Facility, FlatRide, Heading, Land, Money, Park, Ride, RideStats, Scenery, Segment, Shop,
    Terrain, Track, TrackPiece,
};

impl Park {
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

    /// Puts the price up by one step on whatever is on `tile` — a shop, or the
    /// ride whose track runs across it.
    ///
    /// # Errors
    ///
    /// Fails if there is nothing on that tile to put a price on.
    pub fn raise_the_price_at(&mut self, tile: TilePos) -> Result<Money> {
        if let Some(at) = self.rides.iter().position(|ride| ride.occupies(tile)) {
            return Ok(self.rides[at].raise_price());
        }

        self.raise_price(tile)
    }

    /// Brings it down by one step, on a shop or a ride alike.
    ///
    /// # Errors
    ///
    /// Fails if there is nothing on that tile to put a price on.
    pub fn lower_the_price_at(&mut self, tile: TilePos) -> Result<Money> {
        if let Some(at) = self.rides.iter().position(|ride| ride.occupies(tile)) {
            return Ok(self.rides[at].lower_price());
        }

        self.lower_price(tile)
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
        let ground = self
            .land
            .ground(tile)
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
        anyhow::ensure!(
            self.ride_at(tile).is_none(),
            "there is track across {tile:?}",
        );
        anyhow::ensure!(
            self.scenery_at(tile).is_none(),
            "there is scenery on {tile:?}",
        );

        Ok(())
    }

    /// Puts a facility up without charging for it.
    pub(super) fn put_up(&mut self, tile: TilePos, facility: Facility) -> Result<()> {
        self.check_ground(tile, facility)?;
        self.facilities.replace(tile, Some(Shop::new(facility)));
        Ok(())
    }

    /// Puts up a piece of scenery, taking its cost out of the bank.
    ///
    /// # Errors
    ///
    /// Fails if the park is bankrupt or cannot pay, the tile is outside it, the
    /// ground will not take it, or something is already standing there —
    /// including a guest, since scenery blocks the tile it stands on and a guest
    /// planted under a tree has nowhere to go.
    ///
    /// ```
    /// # use openpark::park::{Park, Scenery};
    /// # use isogrid::iso::TilePos;
    /// let mut park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// let before = park.cash();
    /// let tile = TilePos::new(3, 3);
    ///
    /// park.plant(tile, Scenery::Tree)?;
    /// assert_eq!(park.scenery_at(tile), Some(Scenery::Tree));
    /// assert_eq!(park.cash(), before - Scenery::Tree.cost());
    /// assert!(park.plant(tile, Scenery::Tree).is_err(), "it is taken");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn plant(&mut self, tile: TilePos, scenery: Scenery) -> Result<()> {
        self.check_planting(tile, scenery)?;

        self.scenery.replace(tile, Some(scenery));
        self.adjust_cash(-scenery.cost());
        Ok(())
    }

    /// Whether [`Park::plant`] would be allowed here, for a cursor that wants to
    /// say so before the click.
    pub fn can_plant(&self, tile: TilePos, scenery: Scenery) -> bool {
        self.check_planting(tile, scenery).is_ok()
    }

    /// Takes a piece of scenery down again, returning what was there.
    pub fn uproot(&mut self, tile: TilePos) -> Option<Scenery> {
        self.scenery.replace(tile, None).flatten()
    }

    /// Why a tile will not take scenery.
    fn check_planting(&self, tile: TilePos, scenery: Scenery) -> Result<()> {
        anyhow::ensure!(
            !self.is_bankrupt(),
            "the park is bankrupt and cannot buy anything",
        );
        anyhow::ensure!(
            self.cash >= scenery.cost(),
            "a {} costs {} and the park has {}",
            scenery.name().to_lowercase(),
            scenery.cost(),
            self.cash,
        );
        anyhow::ensure!(
            self.scenery_at(tile).is_none(),
            "there is already a {} on {tile:?}",
            self.scenery_at(tile).map_or("something", Scenery::name),
        );
        anyhow::ensure!(
            !self.is_anybody_on(tile),
            "somebody is standing on {tile:?}",
        );

        self.check_ground_for_scenery(tile)
    }

    /// Why the ground on `tile` will not take scenery, money aside.
    fn check_ground_for_scenery(&self, tile: TilePos) -> Result<()> {
        let ground = self
            .land
            .ground(tile)
            .with_context(|| format!("{tile:?} is outside the park"))?;

        anyhow::ensure!(ground.is_buildable(), "nothing will grow on {ground:?}");
        anyhow::ensure!(
            self.facility_at(tile).is_none(),
            "there is already something on {tile:?}",
        );
        anyhow::ensure!(
            self.ride_at(tile).is_none(),
            "there is track across {tile:?}",
        );

        Ok(())
    }

    /// Takes a facility down again, returning what was there.
    ///
    /// Nothing comes back for it: a demolished stall is a loss, which is what
    /// makes building one a decision.
    pub fn demolish(&mut self, tile: TilePos) -> Option<Shop> {
        self.facilities.replace(tile, None).flatten()
    }

    /// Starts a new ride, with its first piece to be laid on `tile`.
    ///
    /// Nothing is charged yet: a ride with no track on it costs nothing, which
    /// is what makes putting one down and thinking about it free.
    ///
    /// # Errors
    ///
    /// Fails if the park is bankrupt, already has [`Park::MAX_RIDES`], or the
    /// tile will not take track.
    pub fn start_a_ride(
        &mut self,
        name: impl Into<String>,
        tile: TilePos,
        heading: Heading,
    ) -> Result<u32> {
        anyhow::ensure!(
            !self.is_bankrupt(),
            "the park is bankrupt and cannot build anything",
        );
        anyhow::ensure!(
            self.rides.len() < Self::MAX_RIDES,
            "a park cannot hold more than {} rides",
            Self::MAX_RIDES,
        );

        let ground = self
            .land
            .height_at(tile)
            .with_context(|| format!("{tile:?} is outside the park"))?;
        self.check_track_tile(tile, None)?;

        let id = self.next_ride_id;
        self.next_ride_id = self.next_ride_id.wrapping_add(1);
        self.rides.push(Ride::new(
            id,
            name,
            Track::starting_at(tile, heading, ground),
        ));

        Ok(id)
    }

    /// Buys a flat ride and stands it with its corner on `tile`.
    ///
    /// Charged and open in one go: there is no layout to get wrong, so there is
    /// nothing to test either. All that is left to decide is what to charge.
    ///
    /// # Errors
    ///
    /// Fails if the park is bankrupt, cannot pay, already has
    /// [`Park::MAX_RIDES`], or the square of land it needs will not take it.
    ///
    /// ```
    /// # use openpark::park::{FlatRide, Park};
    /// # use isogrid::iso::TilePos;
    /// let mut park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// park.adjust_cash(5_000);
    /// let before = park.cash();
    ///
    /// let id = park.buy_a_ride("The Carousel", FlatRide::Carousel, TilePos::new(4, 4))?;
    /// assert_eq!(park.cash(), before - FlatRide::Carousel.cost());
    /// assert!(park.ride(id).is_some_and(|ride| ride.is_open()), "it should be running");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn buy_a_ride(
        &mut self,
        name: impl Into<String>,
        kind: FlatRide,
        tile: TilePos,
    ) -> Result<u32> {
        self.check_flat_ride(tile, kind)?;

        let id = self.next_ride_id;
        self.next_ride_id = self.next_ride_id.wrapping_add(1);

        let mut ride = Ride::bought(id, name, kind, tile);
        ride.test().context("a bought ride would not run")?;
        ride.open().context("a bought ride would not open")?;

        self.rides.push(ride);
        self.adjust_cash(-kind.cost());
        Ok(id)
    }

    /// Whether [`Park::buy_a_ride`] would be allowed here.
    pub fn can_buy_a_ride(&self, tile: TilePos, kind: FlatRide) -> bool {
        self.check_flat_ride(tile, kind).is_ok()
    }

    /// Why a flat ride will not go here.
    ///
    /// Its whole footprint is checked, not just the corner: half a carousel on
    /// buildable ground is no more use than none of one.
    fn check_flat_ride(&self, tile: TilePos, kind: FlatRide) -> Result<()> {
        anyhow::ensure!(
            !self.is_bankrupt(),
            "the park is bankrupt and cannot buy anything",
        );
        anyhow::ensure!(
            self.rides.len() < Self::MAX_RIDES,
            "a park cannot hold more than {} rides",
            Self::MAX_RIDES,
        );
        anyhow::ensure!(
            self.cash >= kind.cost(),
            "a {} costs {} and the park has {}",
            kind.name().to_lowercase(),
            kind.cost(),
            self.cash,
        );

        let ground = self
            .land
            .height_at(tile)
            .with_context(|| format!("{tile:?} is outside the park"))?;

        #[allow(clippy::cast_possible_wrap)]
        let side = kind.footprint() as i32;
        for dy in 0..side {
            for dx in 0..side {
                let under = tile.offset(dx, dy);
                self.check_track_tile(under, None)?;
                anyhow::ensure!(
                    self.land.height_at(under) == Some(ground),
                    "a {} needs {side} by {side} tiles of level ground, and {under:?} is not level with {tile:?}",
                    kind.name().to_lowercase(),
                );
            }
        }

        Ok(())
    }

    /// Whether [`Park::start_a_ride`] would be allowed here, for a cursor that
    /// wants to say so before the click.
    pub fn can_start_a_ride(&self, tile: TilePos) -> bool {
        !self.is_bankrupt()
            && self.rides.len() < Self::MAX_RIDES
            && self.check_track_tile(tile, None).is_ok()
    }

    /// Lays one more piece on a ride, and charges for it.
    ///
    /// # Errors
    ///
    /// Fails if there is no such ride, the park cannot pay, or the piece will
    /// not go where the layout wants to put it — outside the park, on another
    /// ride, on something built, or underground.
    pub fn lay_track(&mut self, id: u32, piece: TrackPiece) -> Result<Segment> {
        anyhow::ensure!(
            !self.is_bankrupt(),
            "the park is bankrupt and cannot build anything",
        );
        anyhow::ensure!(
            self.cash >= piece.cost(),
            "a {} costs {} and the park has {}",
            piece.name().to_lowercase(),
            piece.cost(),
            self.cash,
        );

        let at = self.index_of_ride(id)?;
        let (tile, _, entry) = self.rides[at]
            .track()
            .map(Track::next_place)
            .with_context(|| format!("{} is not a ride you lay track on", self.rides[at].name()))?;
        let exit = entry + piece.climb();

        self.check_track_tile(tile, Some(id))?;
        let ground = self
            .land
            .height_at(tile)
            .with_context(|| format!("{tile:?} is outside the park"))?;
        anyhow::ensure!(
            entry.min(exit) >= ground,
            "track cannot run underground: {tile:?} stands {ground} steps up",
        );
        if piece == TrackPiece::Station {
            anyhow::ensure!(
                entry == ground,
                "a station has to be at ground level for anybody to reach it",
            );
        }

        let laid = self.rides[at]
            .track_mut()
            .context("a ride that took a piece of track has no track")?
            .push(piece)?;
        self.adjust_cash(-piece.cost());
        Ok(laid)
    }

    /// Takes the last piece of a ride's track back off. Nothing comes back for
    /// it.
    pub fn unlay_track(&mut self, id: u32) -> Option<TrackPiece> {
        let at = self.index_of_ride(id).ok()?;
        self.rides[at].track_mut()?.pop()
    }

    /// Sends a test train round a ride's layout.
    ///
    /// # Errors
    ///
    /// Fails if there is no such ride, or if the layout will not do — see
    /// [`Ride::test`].
    pub fn test_ride(&mut self, id: u32) -> Result<RideStats> {
        let at = self.index_of_ride(id)?;
        let stats = self.rides[at].test()?;
        Ok(stats)
    }

    /// Opens a ride to the queue.
    ///
    /// # Errors
    ///
    /// Fails if there is no such ride, or it cannot open — see [`Ride::open`].
    pub fn open_ride(&mut self, id: u32) -> Result<()> {
        let at = self.index_of_ride(id)?;
        self.rides[at].open()
    }

    /// Shuts a ride without taking it down.
    ///
    /// # Errors
    ///
    /// Fails if there is no such ride.
    pub fn close_ride(&mut self, id: u32) -> Result<()> {
        let at = self.index_of_ride(id)?;
        self.rides[at].close();
        Ok(())
    }

    /// Changes what a ride charges, returning the price on the board.
    ///
    /// # Errors
    ///
    /// Fails if there is no such ride.
    pub fn set_ride_price(&mut self, id: u32, price: Money) -> Result<Money> {
        let at = self.index_of_ride(id)?;
        Ok(self.rides[at].set_price(price))
    }

    /// Takes a whole ride down, returning what was there.
    ///
    /// Everybody aboard is put back on their feet at the station: a demolished
    /// ride should empty out, not take its riders with it.
    pub fn demolish_ride(&mut self, id: u32) -> Option<Ride> {
        let at = self.index_of_ride(id).ok()?;
        let gone = self.rides.remove(at);

        for guest in &mut self.guests {
            if guest.riding() == Some(id) {
                guest.get_off();
            }
        }

        Some(gone)
    }

    /// Where a ride is in the list, by id.
    pub(super) fn index_of_ride(&self, id: u32) -> Result<usize> {
        self.rides
            .iter()
            .position(|ride| ride.id() == id)
            .with_context(|| format!("there is no ride {id}"))
    }

    /// Why a tile will not take a piece of track.
    ///
    /// `mine` is the ride doing the asking, whose own track is allowed to be
    /// there — [`Track::push`] has its own opinion about crossing itself.
    fn check_track_tile(&self, tile: TilePos, mine: Option<u32>) -> Result<()> {
        anyhow::ensure!(self.land.contains(tile), "{tile:?} is outside the park");
        anyhow::ensure!(
            self.facility_at(tile).is_none(),
            "there is something built on {tile:?}",
        );
        anyhow::ensure!(
            self.land.ground(tile).is_some_and(Terrain::is_buildable)
                || self.land.ground(tile) == Some(Terrain::Path),
            "track cannot be built on {:?}",
            self.land.ground(tile),
        );

        anyhow::ensure!(
            self.scenery_at(tile).is_none(),
            "there is scenery on {tile:?}",
        );

        let somebody_elses = self
            .rides
            .iter()
            .find(|ride| Some(ride.id()) != mine && ride.occupies(tile));
        anyhow::ensure!(
            somebody_elses.is_none(),
            "{tile:?} already has {} running across it",
            somebody_elses.map_or("another ride", Ride::name),
        );

        Ok(())
    }

    /// Raises the tile under the pointer by a step, and charges for it.
    ///
    /// # Errors
    ///
    /// Fails if the park cannot pay, is bankrupt, the tile is outside it, the
    /// land will not go any higher, something is built on it, or somebody is
    /// standing on it. Nothing is changed when it fails.
    ///
    /// ```
    /// # use openpark::park::{Land, Park};
    /// # use isogrid::iso::TilePos;
    /// let mut park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// let before = park.cash();
    /// let tile = TilePos::new(4, 4);
    ///
    /// assert_eq!(park.raise(tile)?, 1);
    /// assert_eq!(park.cash(), before - Park::LANDSCAPING);
    /// assert_eq!(park.lower(tile)?, 0);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn raise(&mut self, tile: TilePos) -> Result<i16> {
        self.check_landscaping(tile, Self::LANDSCAPING)?;

        let height = self.land.raise(tile)?;
        self.adjust_cash(-Self::LANDSCAPING);
        Ok(height)
    }

    /// Digs the tile under the pointer down by a step, and charges for it.
    ///
    /// # Errors
    ///
    /// As [`Park::raise`], but for land that will not go any lower.
    pub fn lower(&mut self, tile: TilePos) -> Result<i16> {
        self.check_landscaping(tile, Self::LANDSCAPING)?;

        let height = self.land.lower(tile)?;
        self.adjust_cash(-Self::LANDSCAPING);
        Ok(height)
    }

    /// Lays a new surface on one tile, and charges for it.
    ///
    /// # Errors
    ///
    /// As [`Park::raise`], plus anything no money will buy — see
    /// [`Terrain::lay_cost`] — and, for ground nobody can stand on, a tile with
    /// somebody already on it.
    ///
    /// ```
    /// # use openpark::park::{Park, Terrain};
    /// # use isogrid::iso::TilePos;
    /// let mut park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// let tile = TilePos::new(4, 4);
    ///
    /// park.lay(tile, Terrain::Path)?;
    /// assert_eq!(park.terrain()[tile], Terrain::Path);
    /// assert!(park.lay(tile, Terrain::Rock).is_err(), "rock is not for sale");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn lay(&mut self, tile: TilePos, terrain: Terrain) -> Result<()> {
        let cost = terrain
            .lay_cost()
            .with_context(|| format!("{} cannot be laid down", terrain.name().to_lowercase()))?;

        self.check_landscaping(tile, cost)?;
        self.land.set_ground(tile, terrain);
        self.adjust_cash(-cost);
        Ok(())
    }

    /// Whether [`Park::raise`] or [`Park::lower`] would be allowed here, for a
    /// cursor that wants to say so before the click.
    pub fn can_reshape(&self, tile: TilePos) -> bool {
        self.check_landscaping(tile, Self::LANDSCAPING).is_ok()
            && (self
                .land
                .height_at(tile)
                .is_some_and(|height| height < Land::MAX_HEIGHT || height > Land::MIN_HEIGHT))
    }

    /// Whether [`Park::lay`] would be allowed here.
    pub fn can_lay(&self, tile: TilePos, terrain: Terrain) -> bool {
        terrain
            .lay_cost()
            .is_some_and(|cost| self.check_landscaping(tile, cost).is_ok())
    }

    /// Why the land cannot be worked here, money and all.
    fn check_landscaping(&self, tile: TilePos, cost: Money) -> Result<()> {
        anyhow::ensure!(
            !self.is_bankrupt(),
            "the park is bankrupt and cannot afford a shovel",
        );
        anyhow::ensure!(self.land.contains(tile), "{tile:?} is outside the park");
        anyhow::ensure!(
            self.cash >= cost,
            "that costs {cost} and the park has {}",
            self.cash,
        );
        anyhow::ensure!(
            self.facility_at(tile).is_none(),
            "there is something built on {tile:?}",
        );
        anyhow::ensure!(
            !self.is_anybody_on(tile),
            "somebody is standing on {tile:?}",
        );
        anyhow::ensure!(
            self.ride_at(tile).is_none(),
            "there is track across {tile:?}",
        );

        Ok(())
    }

    /// Whether anybody — guest or staff — is standing on `tile`.
    ///
    /// The land cannot be worked under somebody's feet. Without this a tile
    /// could be raised into a pillar with a guest marooned on top of it, and a
    /// marooned guest never reaches the gate to go home.
    fn is_anybody_on(&self, tile: TilePos) -> bool {
        self.guests.iter().any(|guest| guest.tile() == tile)
            || self.staff.iter().any(|member| member.tile() == tile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::park::fixtures::{
        bankrupt_park, bare_park_opened_for, opened_for, park_with_a_coaster, run, A_WHOLE_VISIT,
    };
    use crate::park::{Guest, RideState, Shop, StaffKind};

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

    #[test]
    fn moving_the_land_costs_money() {
        let mut park = Park::new("Landscaping", 32, 32, 1).unwrap();
        let tile = TilePos::new(5, 5);
        let before = park.cash();

        assert_eq!(park.raise(tile).unwrap(), 1);
        assert_eq!(park.cash(), before - Park::LANDSCAPING);
        assert_eq!(park.land().height_at(tile), Some(1));

        assert_eq!(park.lower(tile).unwrap(), 0);
        assert_eq!(park.cash(), before - Park::LANDSCAPING * 2);
    }

    #[test]
    fn a_park_that_cannot_pay_cannot_dig() {
        let mut park = Park::new("Skint", 32, 32, 1).unwrap();
        park.adjust_cash(-park.cash());

        assert!(park.raise(TilePos::new(5, 5)).is_err());
        assert!(park.lay(TilePos::new(5, 5), Terrain::Path).is_err());
        assert!(!park.can_reshape(TilePos::new(5, 5)));
    }

    #[test]
    fn the_land_cannot_be_moved_under_somebody_standing_on_it() {
        let mut park = opened_for(Park::SLOWEST_ARRIVALS);
        let standing_on = park.guests()[0].tile();

        assert!(
            park.raise(standing_on).is_err(),
            "a guest was marooned on a pillar"
        );
        assert!(park.lower(standing_on).is_err());
        assert!(park.lay(standing_on, Terrain::Water).is_err());
        assert!(!park.can_reshape(standing_on));
    }

    #[test]
    fn the_land_cannot_be_moved_under_a_building() {
        let mut park = Park::new("Landscaping", 32, 32, 1).unwrap();
        let tile = TilePos::new(5, 5);
        park.build(tile, Facility::Bench).unwrap();

        assert!(park.raise(tile).is_err(), "a bench was put on stilts");
        assert!(!park.can_lay(tile, Terrain::Path));
    }

    #[test]
    fn a_bankrupt_park_cannot_afford_a_shovel() {
        let mut park = bankrupt_park();
        park.adjust_cash(100_000);

        assert!(park.raise(TilePos::new(5, 5)).is_err());
        assert!(park.lay(TilePos::new(5, 5), Terrain::Path).is_err());
    }

    #[test]
    fn laying_a_surface_costs_what_it_says_and_rock_is_not_for_sale() {
        let mut park = Park::new("Paving", 32, 32, 1).unwrap();
        let tile = TilePos::new(6, 6);
        let before = park.cash();

        park.lay(tile, Terrain::Path).unwrap();
        assert_eq!(park.terrain()[tile], Terrain::Path);
        assert_eq!(park.cash(), before - Terrain::Path.lay_cost().unwrap());

        assert!(park.lay(tile, Terrain::Rock).is_err());
        assert!(!park.can_lay(tile, Terrain::Rock));
        assert!(park.lay(TilePos::new(-1, -1), Terrain::Path).is_err());
    }

    #[test]
    fn the_land_survives_a_save_with_its_hills() {
        let mut park = Park::new("Hilly", 32, 32, 1).unwrap();
        park.raise(TilePos::new(7, 7)).unwrap();
        park.raise(TilePos::new(7, 7)).unwrap();

        let json = serde_json::to_string(&park).unwrap();
        let loaded: Park = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, park);
        assert_eq!(loaded.land().height_at(TilePos::new(7, 7)), Some(2));
    }

    #[test]
    fn a_ride_costs_what_its_track_costs() {
        let (park, id) = park_with_a_coaster();
        let ride = park.ride(id).expect("the ride is there");

        assert_eq!(
            ride.track().expect("a coaster").pieces().len(),
            12,
            "four corners and eight sides"
        );
        assert!(ride.layout().cost() > 0);
        assert!(
            park.wage_bill() >= ride.upkeep(),
            "a ride costs nothing to run"
        );
        assert!(park.cash() < 50_000 + Park::STARTING_CASH, "it was free");
    }

    #[test]
    fn a_station_has_to_be_somewhere_guests_can_reach() {
        let mut park = Park::new("Rides", 32, 32, 5).unwrap();
        park.adjust_cash(50_000);

        let start = TilePos::new(18, 16);
        park.lay(start, Terrain::Grass).unwrap();
        let id = park.start_a_ride("Up There", start, Heading::East).unwrap();

        park.lay_track(id, TrackPiece::LiftHill).unwrap();
        assert!(
            park.lay_track(id, TrackPiece::Station).is_err(),
            "a station was built up in the air"
        );
    }

    #[test]
    fn track_cannot_be_laid_on_anything_that_is_already_there() {
        let (mut park, _) = park_with_a_coaster();
        let taken = park.rides()[0].tiles()[0];

        assert!(park.build(taken, Facility::Bench).is_err(), "on the track");
        assert!(park.raise(taken).is_err(), "the land under the track moved");

        let another = park
            .start_a_ride("The Other One", taken, Heading::North)
            .err();
        assert!(another.is_some(), "two rides on one tile");
    }

    #[test]
    fn a_ride_cannot_be_laid_underground() {
        let mut park = Park::new("Rides", 32, 32, 5).unwrap();
        park.adjust_cash(50_000);

        let start = TilePos::new(18, 16);
        park.lay(start, Terrain::Grass).unwrap();
        while park.land().height_at(start).unwrap_or(0) > 0 {
            park.lower(start).unwrap();
        }

        let id = park
            .start_a_ride("Down There", start, Heading::East)
            .unwrap();
        park.lay_track(id, TrackPiece::Station).unwrap();
        assert!(
            park.lay_track(id, TrackPiece::SlopeDown).is_err(),
            "the track dug itself into the ground"
        );
    }

    #[test]
    fn guests_queue_for_a_ride_pay_for_it_and_come_back_off() {
        let (park, id) = park_with_a_coaster();
        let park = run(park, A_WHOLE_VISIT);

        let ride = park.ride(id).expect("the ride is there");
        assert!(ride.riders() > 0, "nobody went on it all day");
        assert!(ride.takings() > 0, "and nobody paid");
        assert!(
            park.guests()
                .iter()
                .any(|guest| guest.needs().boredom() < 0.3),
            "nobody in the park had anything to do"
        );
    }

    #[test]
    fn a_park_with_a_ride_keeps_its_guests_longer_than_one_without() {
        let (with_a_ride, _) = park_with_a_coaster();
        let with_a_ride = run(with_a_ride, A_WHOLE_VISIT);
        let without = bare_park_opened_for(A_WHOLE_VISIT);

        assert!(
            with_a_ride.guests_who_left() < without.guests_who_left(),
            "{} left the park with a coaster in it against {} from the empty one",
            with_a_ride.guests_who_left(),
            without.guests_who_left()
        );
    }

    #[test]
    fn nobody_queues_for_a_ride_priced_past_what_it_is_worth() {
        let (mut park, id) = park_with_a_coaster();
        park.set_ride_price(id, Ride::MAX_PRICE).unwrap();

        let park = run(park, A_WHOLE_VISIT);
        assert_eq!(
            park.ride(id).map(Ride::riders),
            Some(0),
            "somebody paid the full hundred"
        );
    }

    #[test]
    fn a_worn_out_ride_breaks_down_and_a_mechanic_puts_it_back() {
        let (mut park, id) = park_with_a_coaster();

        // Worn right out, so the breakdown comes within a visit rather than
        // within an afternoon.
        for _ in 0..200_000 {
            park.tick_once();
            if park.ride(id).map(Ride::state) == Some(RideState::Broken) {
                break;
            }
        }
        assert_eq!(
            park.ride(id).map(Ride::state),
            Some(RideState::Broken),
            "nothing ever went wrong with it"
        );

        // Nobody to fix it: it stays broken.
        let park = run(park, 20_000);
        assert_eq!(park.ride(id).map(Ride::state), Some(RideState::Broken));

        let mut park = park;
        park.adjust_cash(10_000);
        park.hire(StaffKind::Mechanic).unwrap();
        let park = run(park, 20_000);

        assert_ne!(
            park.ride(id).map(Ride::state),
            Some(RideState::Broken),
            "the mechanic never got to it"
        );
    }

    #[test]
    fn demolishing_a_ride_puts_whoever_is_aboard_back_on_their_feet() {
        let (mut park, id) = park_with_a_coaster();
        for _ in 0..A_WHOLE_VISIT {
            park.tick_once();
            if park.guests().iter().any(|guest| guest.riding() == Some(id)) {
                break;
            }
        }
        assert!(
            park.guests().iter().any(|guest| guest.riding() == Some(id)),
            "nobody ever got on"
        );

        park.demolish_ride(id).expect("the ride was there");
        assert!(
            park.guests().iter().all(|guest| guest.riding().is_none()),
            "somebody is still aboard a ride that no longer exists"
        );
        assert!(park.rides().is_empty());
    }

    #[test]
    fn a_bankrupt_park_cannot_build_a_ride() {
        let mut park = bankrupt_park();
        park.adjust_cash(100_000);

        assert!(park
            .start_a_ride("No Chance", TilePos::new(5, 5), Heading::North)
            .is_err());
    }

    #[test]
    fn a_park_survives_a_save_with_a_ride_mid_lap() {
        let (park, id) = park_with_a_coaster();
        let park = run(park, 5_000);

        let json = serde_json::to_string(&park).unwrap();
        let loaded: Park = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded, park);
        assert_eq!(
            loaded.ride(id).map(Ride::stats),
            park.ride(id).map(Ride::stats)
        );
        assert_eq!(
            loaded.ride(id).map(|ride| ride.trains().to_vec()),
            park.ride(id).map(|ride| ride.trains().to_vec())
        );
    }

    #[test]
    fn a_flat_ride_is_bought_open_and_running() {
        let mut park = Park::new("Bought", 32, 32, 5).unwrap();
        park.adjust_cash(10_000);
        let before = park.cash();

        let id = park
            .buy_a_ride("The Carousel", FlatRide::Carousel, TilePos::new(18, 16))
            .expect("there should be room");
        let ride = park.ride(id).expect("it is there");

        assert_eq!(park.cash(), before - FlatRide::Carousel.cost());
        assert!(ride.is_open(), "a bought ride arrives working");
        assert_eq!(ride.flat(), Some(FlatRide::Carousel));
        assert_eq!(ride.track(), None, "there is no track to lay");
        assert_eq!(
            ride.tiles().len(),
            (FlatRide::Carousel.footprint() * FlatRide::Carousel.footprint()) as usize,
            "it should stand on its whole footprint"
        );
    }

    #[test]
    fn a_flat_ride_needs_its_whole_square_of_level_ground() {
        let mut park = Park::new("Bought", 32, 32, 5).unwrap();
        park.adjust_cash(10_000);

        let corner = TilePos::new(18, 16);
        for dy in 0..3 {
            for dx in 0..3 {
                park.lay(corner.offset(dx, dy), Terrain::Grass).unwrap();
                while park.land().height_at(corner.offset(dx, dy)).unwrap_or(0) > 0 {
                    park.lower(corner.offset(dx, dy)).unwrap();
                }
            }
        }
        assert!(park.can_buy_a_ride(corner, FlatRide::HauntedHouse));

        // One tile of the square lifted, and the whole thing is refused.
        park.raise(corner.offset(1, 1)).unwrap();
        assert!(
            !park.can_buy_a_ride(corner, FlatRide::HauntedHouse),
            "half a haunted house went up on a slope"
        );
    }

    #[test]
    fn a_flat_ride_will_not_go_where_something_already_is() {
        let (mut park, _) = park_with_a_coaster();
        park.adjust_cash(10_000);
        let taken = park.rides()[0].tiles()[0];

        assert!(!park.can_buy_a_ride(taken, FlatRide::TeaCups));
        assert!(park.buy_a_ride("Nope", FlatRide::TeaCups, taken).is_err());
    }

    #[test]
    fn a_flat_ride_blocks_everything_else_from_its_land() {
        let mut park = Park::new("Bought", 32, 32, 5).unwrap();
        park.adjust_cash(10_000);
        let corner = TilePos::new(18, 16);
        park.buy_a_ride("The Teacups", FlatRide::TeaCups, corner)
            .expect("there should be room");

        for tile in [corner, corner.offset(1, 1)] {
            assert!(park.build(tile, Facility::Bench).is_err(), "{tile:?}");
            assert!(park.plant(tile, Scenery::Tree).is_err(), "{tile:?}");
            assert!(park.raise(tile).is_err(), "{tile:?}");
        }
    }

    #[test]
    fn the_timid_end_of_the_crowd_rides_the_gentle_things() {
        let mut park = Park::new("Something For Everybody", 32, 32, 5).unwrap();
        park.adjust_cash(20_000);
        let gentle = park
            .buy_a_ride("The Carousel", FlatRide::Carousel, TilePos::new(18, 16))
            .expect("there should be room");

        let park = run(park, A_WHOLE_VISIT);
        let ride = park.ride(gentle).expect("it is there");

        assert!(ride.riders() > 0, "nobody went on the carousel");

        // And the carousel is the sort of thing anybody will go on, which is
        // the whole reason to own one.
        let stats = ride.stats().expect("it was tested");
        assert_eq!(stats.category(), crate::park::Category::Gentle);
        assert!(
            stats.intensity < Guest::NERVE.0,
            "the most timid guest in the park would not go on the carousel"
        );
    }

    #[test]
    fn a_flat_ride_loads_turns_and_lets_everybody_off() {
        let mut park = Park::new("Bought", 32, 32, 5).unwrap();
        park.adjust_cash(20_000);
        let id = park
            .buy_a_ride("The Teacups", FlatRide::TeaCups, TilePos::new(18, 16))
            .unwrap();

        let mut ever_full = false;
        let mut ever_turning = false;
        for _ in 0..A_WHOLE_VISIT {
            park.tick_once();
            let ride = park.ride(id).expect("it is there");

            // Turning: nobody can get on while it is going round.
            if ride.boarding().is_none() && ride.is_open() {
                ever_turning = true;
            }
            if ride.riders() >= FlatRide::TeaCups.seats() {
                ever_full = true;
            }
        }

        assert!(ever_turning, "the teacups never went round");
        assert!(ever_full, "the teacups never filled up");
        assert!(park.ride(id).unwrap().takings() > 0, "and took no money");
    }

    #[test]
    fn a_flat_ride_survives_a_save_mid_turn() {
        let mut park = Park::new("Bought", 32, 32, 5).unwrap();
        park.adjust_cash(20_000);
        let id = park
            .buy_a_ride("The Wheel", FlatRide::FerrisWheel, TilePos::new(18, 16))
            .unwrap();
        let park = run(park, 6_000);

        let json = serde_json::to_string(&park).unwrap();
        let loaded: Park = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded, park);
        assert_eq!(
            loaded.ride(id).map(Ride::layout),
            park.ride(id).map(Ride::layout)
        );
    }
}
