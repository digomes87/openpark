//! Things put up to be looked at rather than used.
//!
//! Scenery is the other half of what a park is for. A guest never queues for a
//! tree, but it walks past one, and a park of bare grass and track earns a
//! worse rating than the same rides with something growing between them.
//!
//! Kept apart from [`crate::park::Facility`] on purpose: a facility answers a
//! need and takes money for it, and scenery does neither. The only thing they
//! share is standing on a tile nobody can walk through.

use isogrid::render::Color;
use serde::{Deserialize, Serialize};

use crate::park::Money;

/// Something put up to be looked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scenery {
    /// A tree. The cheapest thing that makes a park look like one.
    Tree,
    /// A flowerbed: prettier than a tree, and wants more looking after.
    Flowerbed,
    /// A fountain. What a park puts at the end of a path to be looked at.
    Fountain,
    /// A lamp post. Barely decorative on its own, and what makes a row of them
    /// read as a promenade.
    Lamp,
}

impl Scenery {
    /// Everything that can be put up, in the order a toolbar would list it.
    pub const ALL: [Self; 4] = [Self::Tree, Self::Flowerbed, Self::Fountain, Self::Lamp];

    /// What it costs to put one up.
    pub const fn cost(self) -> Money {
        match self {
            Self::Tree => 80,
            Self::Flowerbed => 120,
            Self::Fountain => 450,
            Self::Lamp => 60,
        }
    }

    /// What it costs to keep one for a wage bill.
    ///
    /// A tree looks after itself; a flowerbed does not, and a fountain least of
    /// all. Scenery that cost nothing to keep would be a thing you buy once and
    /// never think about again.
    pub const fn upkeep(self) -> Money {
        match self {
            Self::Tree | Self::Lamp => 1,
            Self::Flowerbed => 3,
            Self::Fountain => 8,
        }
    }

    /// How much better one of these makes the park look, per tile it reaches.
    pub const fn beauty(self) -> u32 {
        match self {
            Self::Tree => 3,
            Self::Flowerbed => 5,
            Self::Fountain => 14,
            Self::Lamp => 1,
        }
    }

    /// How far its looks carry, in tiles.
    ///
    /// A fountain is worth walking to see; a lamp post is worth having where you
    /// are standing.
    pub const fn reach(self) -> u32 {
        match self {
            Self::Tree | Self::Flowerbed => 3,
            Self::Fountain => 6,
            Self::Lamp => 2,
        }
    }

    /// What it draws as.
    pub const fn colour(self) -> Color {
        match self {
            Self::Tree => Color::hex(0x3F_7A_3A),
            Self::Flowerbed => Color::hex(0xD9_5A_8C),
            Self::Fountain => Color::hex(0x7A_C6_E8),
            Self::Lamp => Color::hex(0xE8_D6_8C),
        }
    }

    /// The colour of the trunk, stem or post under it.
    pub const fn stem_colour(self) -> Color {
        match self {
            Self::Tree => Color::hex(0x5B_43_2E),
            Self::Flowerbed => Color::hex(0x4A_6B_3A),
            Self::Fountain => Color::hex(0xB2_B7_BA),
            Self::Lamp => Color::hex(0x4A_4A_4A),
        }
    }

    /// How tall it stands, as a fraction of a tile's width on screen.
    pub const fn height(self) -> f32 {
        match self {
            Self::Tree => 0.7,
            Self::Flowerbed => 0.12,
            Self::Fountain => 0.35,
            Self::Lamp => 0.5,
        }
    }

    /// What to call it in the HUD.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tree => "Tree",
            Self::Flowerbed => "Flowerbed",
            Self::Fountain => "Fountain",
            Self::Lamp => "Lamp",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_free_to_put_up_or_to_keep() {
        for scenery in Scenery::ALL {
            assert!(scenery.cost() > 0, "{scenery:?} is free");
            assert!(scenery.upkeep() > 0, "{scenery:?} costs nothing to keep");
        }
    }

    #[test]
    fn everything_is_worth_looking_at() {
        for scenery in Scenery::ALL {
            assert!(scenery.beauty() > 0, "{scenery:?} is not worth looking at");
            assert!(scenery.reach() > 0, "{scenery:?} cannot be seen at all");
        }
    }

    #[test]
    fn the_dearer_it_is_the_better_it_looks() {
        let mut by_cost = Scenery::ALL;
        by_cost.sort_by_key(|scenery| scenery.cost());

        let looks: Vec<u32> = by_cost.iter().map(|scenery| scenery.beauty()).collect();
        let mut sorted = looks.clone();
        sorted.sort_unstable();

        assert_eq!(
            looks, sorted,
            "something cheap is prettier than something dear"
        );
    }

    #[test]
    fn everything_is_told_apart_by_name_and_colour() {
        let mut names: Vec<_> = Scenery::ALL.iter().map(|s| s.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Scenery::ALL.len());

        let mut colours: Vec<_> = Scenery::ALL.iter().map(|s| s.colour()).collect();
        colours.sort_by_key(|c| (c.r, c.g, c.b));
        colours.dedup();
        assert_eq!(colours.len(), Scenery::ALL.len());
    }

    #[test]
    fn everything_stands_above_the_ground_and_below_a_whole_tile() {
        for scenery in Scenery::ALL {
            assert!((0.0..1.0).contains(&scenery.height()), "{scenery:?}");
        }
    }

    #[test]
    fn scenery_survives_a_save() {
        for scenery in Scenery::ALL {
            let json = serde_json::to_string(&scenery).unwrap();
            assert_eq!(serde_json::from_str::<Scenery>(&json).unwrap(), scenery);
        }
    }
}
