//! The books: what the park is paid, what it pays, and when it runs out.
//!
//! Money is the part of a park that is not about tiles at all, so it sits apart
//! from the land and the crowd. Everything here either moves the bank balance or
//! decides what that balance means — including the one decision a park cannot
//! come back from.

use anyhow::Result;
use isogrid::iso::TilePos;
use isogrid::time::Tick;

use crate::park::{Money, Park, Plan, Ride, Staff, StaffKind};

impl Park {
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

        let rides: Money = self.rides.iter().map(Ride::upkeep).sum();

        wages.saturating_add(upkeep).saturating_add(rides)
    }

    /// Everything every till has taken since the park opened, rides included.
    pub fn takings(&self) -> Money {
        let shops: Money = self
            .facilities
            .iter()
            .filter_map(|(_, built)| built.as_ref())
            .map(|shop| shop.takings())
            .sum();
        let rides: Money = self.rides.iter().map(Ride::takings).sum();

        shops.saturating_add(rides)
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

    /// Pays every wage and every bit of upkeep, once a wage bill is due.
    pub(super) fn pay_the_bills(&mut self) {
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
    pub(super) fn check_solvency(&mut self) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::park::fixtures::{bankrupt_park, bare_park_opened_for, dirt, run, A_WHOLE_VISIT};
    use crate::park::{Facility, Guest, Terrain};
    use isogrid::iso::TilePos;

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
}
