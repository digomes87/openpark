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
    /// Sells drinks. Quicker to use than a meal, and the reason a park needs
    /// toilets.
    DrinkStall,
    /// Somewhere to sit. Free, and the only cure for sore feet.
    Bench,
    /// A toilet. The one thing a guest will not wait for and will not forgive.
    Toilet,
    /// A litter bin. Nobody queues for one; what it does is stop the rubbish
    /// ending up on the path.
    Bin,
}

impl Facility {
    /// Everything that can be built, for tests and for menus later on.
    pub const ALL: [Self; 5] = [
        Self::FoodStall,
        Self::DrinkStall,
        Self::Bench,
        Self::Toilet,
        Self::Bin,
    ];

    /// The things a guest will go out of its way for.
    ///
    /// Everything except the bin, which is used in passing or not at all.
    pub const WANTED: [Self; 4] = [Self::FoodStall, Self::DrinkStall, Self::Bench, Self::Toilet];

    /// What it costs the park to put one up.
    pub const fn build_cost(self) -> Money {
        match self {
            Self::FoodStall => 600,
            Self::DrinkStall => 450,
            Self::Bench => 50,
            Self::Toilet => 700,
            Self::Bin => 40,
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
            Self::DrinkStall => 8,
            // A park that charges for a bench, or for the toilets, may: both
            // open free, and both can be priced like anything else.
            Self::Bench | Self::Toilet | Self::Bin => 0,
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
            Self::DrinkStall => 18,
            Self::Bench => 2,
            // Somebody has to clean them, whatever the park charges.
            Self::Toilet => 30,
            Self::Bin => 1,
        }
    }

    /// How many ticks a guest spends using it.
    ///
    /// At [`isogrid::time::TickRate::CLASSIC`], eight seconds for a meal and
    /// twenty for a proper sit down.
    pub const fn ticks_to_use(self) -> u64 {
        match self {
            Self::FoodStall => 320,
            Self::DrinkStall => 160,
            Self::Bench => 800,
            Self::Toilet => 240,
            // Nobody stands at a bin.
            Self::Bin => 1,
        }
    }

    /// What it draws as, on the tile it stands on.
    pub const fn colour(self) -> Color {
        match self {
            Self::FoodStall => Color::hex(0xC4_5A_3B),
            Self::DrinkStall => Color::hex(0x3B_8C_C4),
            Self::Bench => Color::hex(0x8A_6A_46),
            Self::Toilet => Color::hex(0xD8_D8_D0),
            Self::Bin => Color::hex(0x5A_5A_52),
        }
    }

    /// The colour of the roof or back drawn above the tile.
    pub const fn trim_colour(self) -> Color {
        match self {
            Self::FoodStall => Color::hex(0xF2_E4_C9),
            Self::DrinkStall => Color::hex(0xC9_E8_F2),
            Self::Bench => Color::hex(0xB8_92_63),
            Self::Toilet => Color::hex(0x8C_A8_B2),
            Self::Bin => Color::hex(0x3A_3A_34),
        }
    }

    /// How tall it stands, as a fraction of a tile's width on screen.
    pub const fn height(self) -> f32 {
        match self {
            Self::FoodStall => 0.55,
            Self::DrinkStall => 0.5,
            Self::Bench => 0.18,
            Self::Toilet => 0.65,
            Self::Bin => 0.2,
        }
    }

    /// What to call it in the HUD.
    pub const fn name(self) -> &'static str {
        match self {
            Self::FoodStall => "Food stall",
            Self::DrinkStall => "Drink stall",
            Self::Bench => "Bench",
            Self::Toilet => "Toilet",
            Self::Bin => "Litter bin",
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
    fn the_stalls_charge_and_the_rest_do_not() {
        assert!(Facility::FoodStall.price() > 0);
        assert!(Facility::DrinkStall.price() > 0);

        for free in [Facility::Bench, Facility::Toilet, Facility::Bin] {
            assert_eq!(free.price(), 0, "{free:?} opens free");
        }
    }

    #[test]
    fn a_bin_is_not_something_anybody_queues_for() {
        assert!(
            !Facility::WANTED.contains(&Facility::Bin),
            "guests queue for the bins"
        );
        for wanted in Facility::WANTED {
            assert!(Facility::ALL.contains(&wanted), "{wanted:?}");
        }
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
        for stall in [Facility::FoodStall, Facility::DrinkStall] {
            assert!(
                stall.price() * 3 > stall.upkeep(),
                "three customers a wage bill should keep a {stall:?} in business"
            );
        }
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
        for stall in [Facility::FoodStall, Facility::DrinkStall] {
            assert!(
                stall.build_cost() / stall.price() < 100,
                "a {stall:?} would need a hundred customers to break even"
            );
        }
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
