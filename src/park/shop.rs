//! A facility once it has been built, and the till it keeps.
//!
//! [`Facility`] says what a kind of thing is like; a [`Shop`] is one actual
//! stall or bench standing on a tile, with a price its owner set and a record
//! of what it has taken. Two food stalls in the same park can charge different
//! amounts, which is the whole point of being allowed to set a price.

use serde::{Deserialize, Serialize};

use crate::park::{Facility, Money};

/// One built facility.
///
/// ```
/// # use openpark::park::{Facility, Shop};
/// let mut shop = Shop::new(Facility::FoodStall);
/// assert_eq!(shop.price(), Facility::FoodStall.price(), "it opens at the usual price");
///
/// shop.set_price(30);
/// shop.take(30);
/// assert_eq!(shop.takings(), 30);
/// assert_eq!(shop.customers(), 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shop {
    kind: Facility,
    price: Money,
    takings: Money,
    customers: u32,
}

impl Shop {
    /// The most that can be charged for anything.
    ///
    /// Nothing stops an owner pricing a bench out of reach of every guest in
    /// the park; this only stops the number running away entirely.
    pub const MAX_PRICE: Money = 100;

    /// How much one nudge of the price tool moves it.
    pub const PRICE_STEP: Money = 2;

    /// Opens one, at the price its kind usually charges.
    pub const fn new(kind: Facility) -> Self {
        Self {
            kind,
            price: kind.price(),
            takings: 0,
            customers: 0,
        }
    }

    /// What kind of thing this is.
    pub const fn kind(self) -> Facility {
        self.kind
    }

    /// What it charges a guest.
    pub const fn price(self) -> Money {
        self.price
    }

    /// Everything it has taken since it opened.
    pub const fn takings(self) -> Money {
        self.takings
    }

    /// How many guests have paid it.
    pub const fn customers(self) -> u32 {
        self.customers
    }

    /// Sets the price, clamped to something a park can actually charge.
    ///
    /// Returns the price that ended up on the board, which is not always the
    /// one asked for.
    ///
    /// ```
    /// # use openpark::park::{Facility, Shop};
    /// let mut shop = Shop::new(Facility::Bench);
    /// assert_eq!(shop.set_price(-5), 0, "nobody is paid to sit down");
    /// assert_eq!(shop.set_price(10_000), Shop::MAX_PRICE);
    /// ```
    pub fn set_price(&mut self, price: Money) -> Money {
        self.price = price.clamp(0, Self::MAX_PRICE);
        self.price
    }

    /// Puts the price up by one step.
    pub fn raise_price(&mut self) -> Money {
        self.set_price(self.price.saturating_add(Self::PRICE_STEP))
    }

    /// Brings the price down by one step.
    pub fn lower_price(&mut self) -> Money {
        self.set_price(self.price.saturating_sub(Self::PRICE_STEP))
    }

    /// Records a sale of `amount`.
    ///
    /// The amount is passed in rather than read off the price so that the till
    /// records what the guest actually handed over, even if the price changes
    /// in the same tick.
    pub fn take(&mut self, amount: Money) {
        self.takings = self.takings.saturating_add(amount);
        self.customers = self.customers.saturating_add(1);
    }

    /// What it costs the park to keep this open for one wage bill.
    pub const fn upkeep(self) -> Money {
        self.kind.upkeep()
    }

    /// Whether the price is more than guests of this kind will stand.
    ///
    /// A shop over the odds is not refused outright — a hungry guest with money
    /// may still pay and resent it — but it is what
    /// [`crate::park::Guest::will_pay`] measures its patience against.
    pub const fn is_over_the_odds(self) -> bool {
        self.price > self.kind.price()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_shop_charges_what_its_kind_charges() {
        for kind in Facility::ALL {
            let shop = Shop::new(kind);
            assert_eq!(shop.price(), kind.price(), "{kind:?}");
            assert_eq!(shop.takings(), 0);
            assert_eq!(shop.customers(), 0);
            assert!(!shop.is_over_the_odds());
        }
    }

    #[test]
    fn a_price_never_leaves_its_range() {
        let mut shop = Shop::new(Facility::FoodStall);
        for _ in 0..1_000 {
            shop.raise_price();
        }
        assert_eq!(shop.price(), Shop::MAX_PRICE);

        for _ in 0..1_000 {
            shop.lower_price();
        }
        assert_eq!(shop.price(), 0, "free, but never a refund");
    }

    #[test]
    fn nudging_the_price_moves_it_by_one_step() {
        let mut shop = Shop::new(Facility::FoodStall);
        let opened_at = shop.price();

        assert_eq!(shop.raise_price(), opened_at + Shop::PRICE_STEP);
        assert_eq!(shop.lower_price(), opened_at);
    }

    #[test]
    fn the_till_adds_up() {
        let mut shop = Shop::new(Facility::FoodStall);
        shop.take(12);
        shop.take(8);
        assert_eq!(shop.takings(), 20);
        assert_eq!(shop.customers(), 2);
    }

    #[test]
    fn charging_over_the_odds_is_noticed() {
        let mut shop = Shop::new(Facility::FoodStall);
        assert!(!shop.is_over_the_odds());
        shop.raise_price();
        assert!(shop.is_over_the_odds());
    }

    #[test]
    fn a_shop_survives_a_save() {
        let mut shop = Shop::new(Facility::FoodStall);
        shop.set_price(19);
        shop.take(19);

        let json = serde_json::to_string(&shop).unwrap();
        assert_eq!(serde_json::from_str::<Shop>(&json).unwrap(), shop);
    }
}
