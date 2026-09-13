//! Rides with no track to lay.
//!
//! A coaster is a shape you build; a carousel is a thing you buy. It stands on
//! its own tiles, turns for a while with whoever is aboard, and stops. There is
//! no layout to get wrong, which is the point of them: a park needs something
//! for the guests who will not go near a lift hill, and a player needs something
//! to put down on the first afternoon.
//!
//! What they are like is fixed rather than measured. A coaster earns its
//! excitement from its own drops, and there is nothing for a carousel to earn it
//! with — so the numbers are written down here, and the only decision left is
//! what to charge.

use isogrid::render::Color;
use serde::{Deserialize, Serialize};

use crate::park::Money;

/// A ride that comes as it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlatRide {
    /// A carousel. Gentle enough for anybody, and dull enough that nobody
    /// queues twice.
    Carousel,
    /// A ferris wheel: slow, tall, and worth the view.
    FerrisWheel,
    /// A haunted house. Not rough at all, and frightening all the same.
    HauntedHouse,
    /// Spinning teacups. Rougher than they look.
    TeaCups,
}

impl FlatRide {
    /// Everything that can be bought, in the order a toolbar would list it.
    pub const ALL: [Self; 4] = [
        Self::Carousel,
        Self::FerrisWheel,
        Self::HauntedHouse,
        Self::TeaCups,
    ];

    /// What it costs to put one up.
    pub const fn cost(self) -> Money {
        match self {
            Self::Carousel => 900,
            Self::FerrisWheel => 1_600,
            Self::HauntedHouse => 1_400,
            Self::TeaCups => 700,
        }
    }

    /// What it costs to keep running for one wage bill.
    pub const fn upkeep(self) -> Money {
        match self {
            Self::Carousel | Self::TeaCups => 12,
            Self::FerrisWheel => 18,
            Self::HauntedHouse => 15,
        }
    }

    /// How many tiles on a side it stands on.
    ///
    /// A carousel is a two-by-two thing; a haunted house is a building. Nothing
    /// here is one tile: a flat ride that took no more room than a bench would
    /// make the land free.
    pub const fn footprint(self) -> u32 {
        match self {
            Self::Carousel | Self::TeaCups => 2,
            Self::FerrisWheel | Self::HauntedHouse => 3,
        }
    }

    /// How many guests it carries at once.
    pub const fn seats(self) -> u32 {
        match self {
            Self::Carousel => 12,
            Self::FerrisWheel => 16,
            Self::HauntedHouse => 10,
            Self::TeaCups => 8,
        }
    }

    /// How many ticks one go lasts.
    pub const fn ride_time(self) -> u32 {
        match self {
            Self::Carousel => 800,
            Self::FerrisWheel => 1_600,
            Self::HauntedHouse => 1_000,
            Self::TeaCups => 600,
        }
    }

    /// How much fun it is, from 0 to 1.
    pub const fn excitement(self) -> f32 {
        match self {
            Self::Carousel => 0.22,
            Self::FerrisWheel => 0.4,
            Self::HauntedHouse => 0.55,
            Self::TeaCups => 0.3,
        }
    }

    /// How rough it is, from 0 to 1.
    pub const fn intensity(self) -> f32 {
        match self {
            Self::Carousel => 0.05,
            Self::FerrisWheel => 0.1,
            Self::HauntedHouse => 0.35,
            Self::TeaCups => 0.45,
        }
    }

    /// What it draws as.
    pub const fn colour(self) -> Color {
        match self {
            Self::Carousel => Color::hex(0xE8_C3_4A),
            Self::FerrisWheel => Color::hex(0x4A_9D_E8),
            Self::HauntedHouse => Color::hex(0x6B_4A_8C),
            Self::TeaCups => Color::hex(0xE8_7A_A8),
        }
    }

    /// The colour of the frame under it.
    pub const fn frame_colour(self) -> Color {
        match self {
            Self::Carousel => Color::hex(0xB2_8A_3A),
            Self::FerrisWheel => Color::hex(0xB2_B7_BA),
            Self::HauntedHouse => Color::hex(0x3A_2A_4A),
            Self::TeaCups => Color::hex(0xB2_5A_7A),
        }
    }

    /// How tall it stands, as a fraction of a tile's width on screen.
    pub const fn height(self) -> f32 {
        match self {
            Self::Carousel => 0.7,
            Self::FerrisWheel => 1.6,
            Self::HauntedHouse => 1.0,
            Self::TeaCups => 0.4,
        }
    }

    /// What to call it in the HUD.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Carousel => "Carousel",
            Self::FerrisWheel => "Ferris wheel",
            Self::HauntedHouse => "Haunted house",
            Self::TeaCups => "Teacups",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_costs_money_and_carries_people() {
        for ride in FlatRide::ALL {
            assert!(ride.cost() > 0, "{ride:?} is free");
            assert!(ride.upkeep() > 0, "{ride:?} costs nothing to run");
            assert!(ride.seats() > 0, "{ride:?} carries nobody");
            assert!(ride.ride_time() > 0, "{ride:?} is over instantly");
            assert!(ride.footprint() > 1, "{ride:?} takes up no room");
        }
    }

    #[test]
    fn nothing_is_more_intense_than_it_is_exciting() {
        // Not a law of physics — a deliberate line. A flat ride that punished
        // people more than it entertained them would be a coaster built badly,
        // and there is no way for a player to fix a flat ride.
        for ride in FlatRide::ALL {
            assert!(
                ride.intensity() < ride.excitement() + 0.2,
                "{ride:?} is {} intense against {} exciting",
                ride.intensity(),
                ride.excitement()
            );
        }
    }

    #[test]
    fn every_number_stays_in_its_range() {
        for ride in FlatRide::ALL {
            assert!((0.0..=1.0).contains(&ride.excitement()), "{ride:?}");
            assert!((0.0..=1.0).contains(&ride.intensity()), "{ride:?}");
            assert!(ride.height() > 0.0, "{ride:?} is flat on the ground");
        }
    }

    #[test]
    fn there_is_something_for_the_timid_and_something_for_the_brave() {
        let gentlest = FlatRide::ALL
            .iter()
            .map(|ride| ride.intensity())
            .fold(f32::MAX, f32::min);
        let roughest = FlatRide::ALL
            .iter()
            .map(|ride| ride.intensity())
            .fold(f32::MIN, f32::max);

        assert!(gentlest < 0.1, "nothing here is gentle: {gentlest}");
        assert!(roughest > 0.3, "nothing here has any bite: {roughest}");
    }

    #[test]
    fn everything_is_told_apart_by_name_and_colour() {
        let mut names: Vec<_> = FlatRide::ALL.iter().map(|ride| ride.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), FlatRide::ALL.len());

        let mut colours: Vec<_> = FlatRide::ALL.iter().map(|ride| ride.colour()).collect();
        colours.sort_by_key(|c| (c.r, c.g, c.b));
        colours.dedup();
        assert_eq!(colours.len(), FlatRide::ALL.len());
    }

    #[test]
    fn a_flat_ride_survives_a_save() {
        for ride in FlatRide::ALL {
            let json = serde_json::to_string(&ride).unwrap();
            assert_eq!(serde_json::from_str::<FlatRide>(&json).unwrap(), ride);
        }
    }
}
