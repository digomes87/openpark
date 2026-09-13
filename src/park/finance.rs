//! Borrowing money, and spending it on being talked about.
//!
//! The two things a park can do with money that are not building: take a loan
//! against its future, and buy some of the word of mouth it has not earned.
//! Both are ways of bringing tomorrow's guests forward, and both cost more than
//! they give back — which is the whole decision.

use serde::{Deserialize, Serialize};

use crate::park::{Money, Park, Rating};

/// A marketing campaign: what it is, and how long it has left to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Campaign {
    /// How many ticks of it are left.
    ticks_left: u64,
}

impl Campaign {
    /// What one campaign costs to start.
    pub const COST: Money = 800;

    /// How long one runs for.
    ///
    /// Five wage bills: long enough to fill a park that has something worth
    /// filling it with, and short enough that advertising an empty field is
    /// money thrown away rather than a permanent subsidy.
    pub const LENGTH: u64 = 6_000;

    /// How much of a reputation a campaign lends a park while it runs.
    ///
    /// Lent, not earned: the gate behaves as though people thought better of the
    /// place than they do, and goes back to the truth when the campaign ends.
    pub const BORROWED_REGARD: u32 = 350;

    /// A campaign that has just started.
    pub const fn new() -> Self {
        Self {
            ticks_left: Self::LENGTH,
        }
    }

    /// How many ticks it has left.
    pub const fn ticks_left(&self) -> u64 {
        self.ticks_left
    }

    /// Whether it is still running.
    pub const fn is_running(&self) -> bool {
        self.ticks_left > 0
    }

    /// Runs it for one tick.
    pub const fn tick(&mut self) {
        self.ticks_left = self.ticks_left.saturating_sub(1);
    }
}

impl Default for Campaign {
    fn default() -> Self {
        Self::new()
    }
}

impl Park {
    /// The most a park can owe the bank.
    ///
    /// A flat ceiling rather than a share of what the park is worth: a limit
    /// that grew with the park would lend most to whoever needed it least.
    pub const LOAN_LIMIT: Money = 20_000;

    /// How much a loan can be moved by at a time.
    pub const LOAN_STEP: Money = 1_000;

    /// What the bank charges on the outstanding loan, per wage bill, in
    /// hundredths.
    ///
    /// Two per cent a bill sounds small and is not: a park that borrows the
    /// limit and forgets about it pays the interest of a member of staff every
    /// bill, for ever.
    pub const INTEREST: Money = 2;

    /// What the park owes the bank.
    pub const fn loan(&self) -> Money {
        self.loan
    }

    /// What the park pays the bank each wage bill on what it owes.
    pub fn interest(&self) -> Money {
        self.loan * Self::INTEREST / 100
    }

    /// Borrows `amount`, rounded down to whole [`Park::LOAN_STEP`]s.
    ///
    /// # Errors
    ///
    /// Fails if the park is bankrupt — the bank does not lend to a park it has
    /// already closed — or if the loan would pass [`Park::LOAN_LIMIT`].
    ///
    /// ```
    /// # use openpark::park::Park;
    /// let mut park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// let before = park.cash();
    ///
    /// park.borrow(Park::LOAN_STEP)?;
    /// assert_eq!(park.cash(), before + Park::LOAN_STEP);
    /// assert_eq!(park.loan(), Park::LOAN_STEP);
    /// assert!(park.borrow(Park::LOAN_LIMIT).is_err(), "past the limit");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn borrow(&mut self, amount: Money) -> anyhow::Result<Money> {
        anyhow::ensure!(
            !self.is_bankrupt(),
            "the bank will not lend to a park it has closed",
        );
        anyhow::ensure!(amount > 0, "a loan of nothing is not a loan");
        anyhow::ensure!(
            self.loan + amount <= Self::LOAN_LIMIT,
            "the park owes {} and the bank will not go past {}",
            self.loan,
            Self::LOAN_LIMIT,
        );

        self.loan += amount;
        self.adjust_cash(amount);
        Ok(self.loan)
    }

    /// Pays `amount` off the loan.
    ///
    /// # Errors
    ///
    /// Fails if the park does not owe that much, or cannot pay it.
    pub fn repay(&mut self, amount: Money) -> anyhow::Result<Money> {
        anyhow::ensure!(amount > 0, "paying off nothing is not paying off");
        anyhow::ensure!(amount <= self.loan, "the park only owes {}", self.loan);
        anyhow::ensure!(
            self.cash >= amount,
            "paying off {amount} would leave the park with {}",
            self.cash - amount,
        );

        self.loan -= amount;
        self.adjust_cash(-amount);
        Ok(self.loan)
    }

    /// The campaign that is running, if one is.
    pub const fn campaign(&self) -> Option<Campaign> {
        self.campaign
    }

    /// Starts a marketing campaign, and pays for it up front.
    ///
    /// # Errors
    ///
    /// Fails if the park is bankrupt, cannot pay, or is already advertising —
    /// two campaigns at once buy no more attention than one.
    pub fn advertise(&mut self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.is_bankrupt(),
            "a bankrupt park has nothing to advertise",
        );
        anyhow::ensure!(self.campaign.is_none(), "the park is already advertising");
        anyhow::ensure!(
            self.cash >= Campaign::COST,
            "a campaign costs {} and the park has {}",
            Campaign::COST,
            self.cash,
        );

        self.campaign = Some(Campaign::new());
        self.adjust_cash(-Campaign::COST);
        tracing::info!(cost = Campaign::COST, "started advertising");
        Ok(())
    }

    /// What the gate is going by: what people think, plus whatever the
    /// advertising is lending the place.
    ///
    /// Capped at [`Rating::BEST`], so a campaign cannot make a park better than
    /// the best park there could be — it can only bring a middling one forward.
    pub fn regard(&self) -> u32 {
        let borrowed = if self.campaign.is_some() {
            Campaign::BORROWED_REGARD
        } else {
            0
        };

        (self.reputation + borrowed).min(Rating::BEST)
    }

    /// Runs the advertising and the interest for one tick.
    ///
    /// The interest is only charged when a bill falls due; `billing` says
    /// whether this is that tick.
    pub(super) fn keep_the_books(&mut self, billing: bool) {
        if let Some(campaign) = &mut self.campaign {
            campaign.tick();
            if !campaign.is_running() {
                self.campaign = None;
                tracing::info!("the advertising ran out");
            }
        }

        if billing && self.loan > 0 {
            let interest = self.interest();
            self.adjust_cash(-interest);
            tracing::debug!(interest, owed = self.loan, "paid the bank its interest");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::park::fixtures::{bankrupt_park, run};

    #[test]
    fn a_new_park_owes_nothing_and_advertises_nothing() {
        let park = Park::new("Forest Frontiers", 32, 32, 1).unwrap();

        assert_eq!(park.loan(), 0);
        assert_eq!(park.interest(), 0);
        assert_eq!(park.campaign(), None);
        assert_eq!(
            park.regard(),
            park.reputation,
            "nothing lent, nothing added"
        );
    }

    #[test]
    fn borrowing_puts_money_in_the_bank_and_a_debt_on_the_books() {
        let mut park = Park::new("Borrowed", 32, 32, 1).unwrap();
        let before = park.cash();

        assert_eq!(park.borrow(Park::LOAN_STEP).unwrap(), Park::LOAN_STEP);
        assert_eq!(park.cash(), before + Park::LOAN_STEP);
        assert_eq!(park.loan(), Park::LOAN_STEP);
        assert!(park.interest() > 0, "the bank lends for nothing");
    }

    #[test]
    fn the_bank_has_a_limit_and_will_not_lend_nothing() {
        let mut park = Park::new("Borrowed", 32, 32, 1).unwrap();

        park.borrow(Park::LOAN_LIMIT).unwrap();
        assert_eq!(park.loan(), Park::LOAN_LIMIT);
        assert!(park.borrow(Park::LOAN_STEP).is_err(), "past the limit");
        assert!(park.borrow(0).is_err(), "a loan of nothing");
        assert!(park.borrow(-500).is_err(), "a negative loan");
    }

    #[test]
    fn paying_a_loan_off_takes_it_out_of_the_bank() {
        let mut park = Park::new("Borrowed", 32, 32, 1).unwrap();
        park.borrow(Park::LOAN_STEP * 2).unwrap();
        let flush = park.cash();

        assert_eq!(park.repay(Park::LOAN_STEP).unwrap(), Park::LOAN_STEP);
        assert_eq!(park.cash(), flush - Park::LOAN_STEP);
        assert!(
            park.repay(Park::LOAN_STEP * 5).is_err(),
            "more than it owes"
        );
    }

    #[test]
    fn a_park_cannot_pay_off_a_loan_it_cannot_afford() {
        let mut park = Park::new("Skint", 32, 32, 1).unwrap();
        park.borrow(Park::LOAN_STEP).unwrap();
        park.adjust_cash(-park.cash());

        assert!(park.repay(Park::LOAN_STEP).is_err());
        assert_eq!(park.loan(), Park::LOAN_STEP, "it owes what it owed");
    }

    #[test]
    fn interest_comes_out_at_every_bill() {
        let mut park = Park::new("Borrowed", 32, 32, 1).unwrap();
        for tile in park.terrain().positions().collect::<Vec<_>>() {
            park.demolish(tile);
        }
        park.borrow(Park::LOAN_LIMIT).unwrap();

        let owed = park.interest();
        assert!(owed > 0);

        let before = park.cash();
        let park = run(park, Park::TICKS_PER_WAGE_BILL);

        // Whatever came in at the gate, the interest went out — and the loan
        // itself is untouched, which is the trap.
        let through_the_gate =
            Money::try_from(park.guests().len() + park.guests_who_left() as usize).unwrap();
        assert_eq!(
            park.cash(),
            before + Park::ADMISSION * through_the_gate - owed,
        );
        assert_eq!(park.loan(), Park::LOAN_LIMIT, "the debt paid itself off");
    }

    #[test]
    fn the_bank_will_not_lend_to_a_park_it_has_closed() {
        let mut park = bankrupt_park();

        assert!(park.borrow(Park::LOAN_STEP).is_err());
        assert!(park.advertise().is_err());
    }

    #[test]
    fn advertising_costs_money_and_runs_out() {
        let mut park = Park::new("Advertised", 32, 32, 1).unwrap();
        let before = park.cash();

        park.advertise().expect("it should start");
        assert_eq!(park.cash(), before - Campaign::COST);
        assert!(park
            .campaign()
            .is_some_and(|campaign| campaign.is_running()));
        assert!(park.advertise().is_err(), "two campaigns at once");

        let park = run(park, Campaign::LENGTH + 1);
        assert_eq!(park.campaign(), None, "the campaign never ended");
    }

    #[test]
    fn advertising_lends_the_park_a_reputation_it_has_not_earned() {
        let mut park = Park::new("Advertised", 32, 32, 1).unwrap();
        let honest = park.regard();

        park.advertise().unwrap();
        assert!(park.regard() > honest, "the advertising bought nothing");
        assert!(park.regard() <= Rating::BEST, "past the best there is");
    }

    #[test]
    fn advertising_fills_a_park_faster_than_waiting_does() {
        let mut advertised = Park::new("Advertised", 32, 32, 5).unwrap();
        let quiet = Park::new("Quiet", 32, 32, 5).unwrap();

        advertised.advertise().unwrap();
        let advertised = run(advertised, Campaign::LENGTH);
        let quiet = run(quiet, Campaign::LENGTH);

        assert!(
            advertised.guests().len() > quiet.guests().len(),
            "{} turned up to the advertised park against {} to the quiet one",
            advertised.guests().len(),
            quiet.guests().len()
        );
    }

    #[test]
    fn the_books_survive_a_save() {
        let mut park = Park::new("Borrowed", 32, 32, 1).unwrap();
        park.borrow(Park::LOAN_STEP * 3).unwrap();
        park.advertise().unwrap();
        let park = run(park, 500);

        let json = serde_json::to_string(&park).unwrap();
        let loaded: Park = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded, park);
        assert_eq!(loaded.loan(), park.loan());
        assert_eq!(loaded.campaign(), park.campaign());
    }
}
