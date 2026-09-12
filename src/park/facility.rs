//! The things a park puts on its land for guests to use.
//!
//! A facility sits *on* a tile rather than being one: the ground underneath is
//! still grass or path, and it is still what a bulldozer would leave behind.
//! Guests use a facility from the tile next to it, which is why a facility
//! blocks the tile it stands on.

use isogrid::render::Color;
use serde::{Deserialize, Serialize};

use crate::park::Money;

/// Something built on a tile.
///
/// Small on purpose: one thing that answers hunger, one that answers tiredness.
/// Rides are a different shape of problem and come later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Facility {
    /// Sells food. The only cure for hunger there is.
    FoodStall,
    /// Somewhere to sit. Free, and the only cure for sore feet.
    Bench,
}

impl Facility {
    /// Everything that can be built, for tests and for menus later on.
    pub const ALL: [Self; 2] = [Self::FoodStall, Self::Bench];

    /// What it costs the park to put one up.
    pub const fn build_cost(self) -> Money {
        match self {
            Self::FoodStall => 600,
            Self::Bench => 50,
        }
    }

    /// The price one opens at, and the price guests think is fair.
    ///
    /// A [`crate::park::Shop`] may charge whatever its owner sets; this is what
    /// it is compared against, both on the day it opens and in the mind of
    /// every guest that walks past it.
    ///
    /// ```
    /// # use openpark::park::Facility;
    /// assert!(Facility::FoodStall.price() > 0, "a stall is a business");
    /// assert_eq!(Facility::Bench.price(), 0, "sitting down is free");
    /// ```
    pub const fn price(self) -> Money {
        match self {
            Self::FoodStall => 12,
            Self::Bench => 0,
        }
    }

    /// What it costs the park to keep one open for one wage bill.
    ///
    /// Stock, cleaning and repairs. A bench costs almost nothing, which is why
    /// a park can afford to line its paths with them; a stall has to sell
    /// something to be worth having.
    ///
    /// ```
    /// # use openpark::park::Facility;
    /// assert!(Facility::FoodStall.upkeep() > Facility::Bench.upkeep());
    /// ```
    pub const fn upkeep(self) -> Money {
        match self {
            Self::FoodStall => 25,
            Self::Bench => 2,
        }
    }

    /// How many ticks a guest spends using it.
    ///
    /// At [`isogrid::time::TickRate::CLASSIC`], eight seconds for a meal and
    /// twenty for a proper sit down.
    pub const fn ticks_to_use(self) -> u64 {
        match self {
            Self::FoodStall => 320,
            Self::Bench => 800,
        }
    }

    /// What it draws as, on the tile it stands on.
    pub const fn colour(self) -> Color {
        match self {
            Self::FoodStall => Color::hex(0xC4_5A_3B),
            Self::Bench => Color::hex(0x8A_6A_46),
        }
    }

    /// The colour of the roof or back drawn above the tile.
    pub const fn trim_colour(self) -> Color {
        match self {
            Self::FoodStall => Color::hex(0xF2_E4_C9),
            Self::Bench => Color::hex(0xB8_92_63),
        }
    }

    /// How tall it stands, as a fraction of a tile's width on screen.
    pub const fn height(self) -> f32 {
        match self {
            Self::FoodStall => 0.55,
            Self::Bench => 0.18,
        }
    }

    /// What to call it in the HUD.
    pub const fn name(self) -> &'static str {
        match self {
            Self::FoodStall => "Food stall",
            Self::Bench => "Bench",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_free_to_build() {
        for facility in Facility::ALL {
            assert!(
                facility.build_cost() > 0,
                "{facility:?} costs the park nothing to put up"
            );
        }
    }

    #[test]
    fn a_stall_charges_and_a_bench_does_not() {
        assert!(Facility::FoodStall.price() > 0);
        assert_eq!(Facility::Bench.price(), 0);
    }

    #[test]
    fn everything_costs_something_to_keep_open() {
        for facility in Facility::ALL {
            assert!(
                facility.upkeep() > 0,
                "{facility:?} costs the park nothing to run, so there is no reason to demolish one"
            );
        }
    }

    #[test]
    fn a_stall_at_the_fair_price_covers_its_own_upkeep() {
        let stall = Facility::FoodStall;
        assert!(
            stall.price() * 3 > stall.upkeep(),
            "three customers a wage bill should keep a stall in business"
        );
    }

    #[test]
    fn using_anything_takes_time() {
        for facility in Facility::ALL {
            assert!(
                facility.ticks_to_use() > 0,
                "{facility:?} is used instantly, which nothing is"
            );
        }
    }

    #[test]
    fn a_stall_pays_for_itself_eventually() {
        let stall = Facility::FoodStall;
        assert!(
            stall.build_cost() / stall.price() < 100,
            "a stall would need a hundred customers to break even"
        );
    }

    #[test]
    fn everything_is_told_apart_by_colour_and_by_name() {
        let mut colours: Vec<_> = Facility::ALL.iter().map(|f| f.colour()).collect();
        colours.sort_by_key(|c| (c.r, c.g, c.b));
        colours.dedup();
        assert_eq!(colours.len(), Facility::ALL.len());

        let mut names: Vec<_> = Facility::ALL.iter().map(|f| f.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Facility::ALL.len());
    }

    #[test]
    fn everything_stands_above_the_ground_and_below_a_whole_tile() {
        for facility in Facility::ALL {
            assert!((0.0..1.0).contains(&facility.height()), "{facility:?}");
        }
    }

    #[test]
    fn a_facility_survives_a_save() {
        for facility in Facility::ALL {
            let json = serde_json::to_string(&facility).unwrap();
            assert_eq!(serde_json::from_str::<Facility>(&json).unwrap(), facility);
        }
        assert_eq!(
            serde_json::to_string(&Facility::FoodStall).unwrap(),
            "\"food_stall\""
        );
    }
}
