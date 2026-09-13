//! What the ground is made of.

use isogrid::render::Color;
use serde::{Deserialize, Serialize};

/// A single kind of ground.
///
/// Deliberately small for now. Rides, scenery and buildings sit *on* tiles
/// rather than being tiles, so this enum only ever describes the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Terrain {
    /// Open ground. Buildable, walkable off-path.
    Grass,
    /// A laid footpath. Where guests actually want to walk.
    Path,
    /// Queue path: a footpath that only leads to a ride, and that guests stand
    /// in line on rather than walk along.
    Queue,
    /// Bare earth, usually where something was demolished.
    Dirt,
    /// Water. Not walkable, not buildable without work.
    Water,
    /// Solid rock. Not walkable, not buildable.
    Rock,
}

impl Terrain {
    /// Everything a player can lay down, in the order a toolbar would list it.
    ///
    /// Rock is not on it: a park can quarry its way around one, never make one.
    pub const LAYABLE: [Self; 5] = [
        Self::Path,
        Self::Queue,
        Self::Grass,
        Self::Dirt,
        Self::Water,
    ];

    /// What it costs the park to lay one tile of this, or `None` for something
    /// no amount of money will buy.
    ///
    /// ```
    /// # use openpark::park::Terrain;
    /// assert!(Terrain::Path.lay_cost() > Terrain::Grass.lay_cost());
    /// assert_eq!(Terrain::Rock.lay_cost(), None, "rock is not for sale");
    /// ```
    pub const fn lay_cost(self) -> Option<crate::park::Money> {
        match self {
            Self::Path => Some(25),
            Self::Queue => Some(30),
            Self::Grass => Some(10),
            Self::Dirt => Some(5),
            Self::Water => Some(50),
            Self::Rock => None,
        }
    }

    /// What to call it in the HUD.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Grass => "Grass",
            Self::Path => "Path",
            Self::Queue => "Queue path",
            Self::Dirt => "Dirt",
            Self::Water => "Water",
            Self::Rock => "Rock",
        }
    }

    /// Whether a guest can stand here.
    ///
    /// ```
    /// # use openpark::park::Terrain;
    /// assert!(Terrain::Path.is_walkable());
    /// assert!(!Terrain::Water.is_walkable());
    /// ```
    pub const fn is_walkable(self) -> bool {
        matches!(self, Self::Grass | Self::Path | Self::Dirt | Self::Queue)
    }

    /// Whether something can be built here without terraforming first.
    pub const fn is_buildable(self) -> bool {
        matches!(self, Self::Grass | Self::Dirt)
    }

    /// How much effort a guest spends crossing this tile.
    ///
    /// Paths are the cheapest, which is what makes a crowd follow them instead
    /// of cutting across the flower beds. `None` means impassable.
    ///
    /// ```
    /// # use openpark::park::Terrain;
    /// assert!(Terrain::Path.walk_cost() < Terrain::Grass.walk_cost());
    /// assert_eq!(Terrain::Rock.walk_cost(), None);
    /// ```
    pub const fn walk_cost(self) -> Option<u32> {
        match self {
            // A queue is a path with a line standing on it: as easy to walk,
            // and no more inviting, so a crowd with somewhere else to be does
            // not cut through one.
            Self::Path | Self::Queue => Some(1),
            Self::Dirt => Some(4),
            Self::Grass => Some(6),
            Self::Water | Self::Rock => None,
        }
    }

    /// The colour this terrain draws as.
    ///
    /// Placeholder art: flat colours until there are sprites. Chosen to read as
    /// a park in daylight rather than as a debug view — the greens are
    /// desaturated enough that path and water stay legible against them.
    pub const fn colour(self) -> Color {
        match self {
            Self::Grass => Color::hex(0x5E_8C_3E),
            Self::Path => Color::hex(0xC9_B6_92),
            Self::Queue => Color::hex(0xB0_9A_C2),
            Self::Dirt => Color::hex(0x8B_6F_4E),
            Self::Water => Color::hex(0x3E_6E_8C),
            Self::Rock => Color::hex(0x6B_6B_6B),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Terrain; 5] = [
        Terrain::Grass,
        Terrain::Path,
        Terrain::Dirt,
        Terrain::Water,
        Terrain::Rock,
    ];

    #[test]
    fn walkable_terrain_always_has_a_cost() {
        for terrain in ALL {
            assert_eq!(
                terrain.is_walkable(),
                terrain.walk_cost().is_some(),
                "{terrain:?} disagrees with itself about being walkable"
            );
        }
    }

    #[test]
    fn no_step_is_free() {
        for terrain in ALL {
            assert_ne!(
                terrain.walk_cost(),
                Some(0),
                "{terrain:?} would let a guest loop for free"
            );
        }
    }

    #[test]
    fn guests_prefer_paths_to_everything_else() {
        let path = Terrain::Path.walk_cost().unwrap();
        for terrain in ALL.into_iter().filter(|t| *t != Terrain::Path) {
            if let Some(cost) = terrain.walk_cost() {
                assert!(
                    path < cost,
                    "guests would not prefer a path over {terrain:?}"
                );
            }
        }
    }

    #[test]
    fn buildable_ground_is_also_walkable() {
        for terrain in ALL.into_iter().filter(|t| t.is_buildable()) {
            assert!(
                terrain.is_walkable(),
                "{terrain:?} can be built on but not stood on"
            );
        }
    }

    #[test]
    fn every_terrain_looks_different() {
        let mut colours: Vec<_> = ALL.into_iter().map(Terrain::colour).collect();
        colours.sort_by_key(|c| (c.r, c.g, c.b));
        colours.dedup();
        assert_eq!(
            colours.len(),
            ALL.len(),
            "two terrains draw the same colour"
        );
    }

    #[test]
    fn terrain_survives_a_save() {
        for terrain in ALL {
            let json = serde_json::to_string(&terrain).unwrap();
            assert_eq!(serde_json::from_str::<Terrain>(&json).unwrap(), terrain);
        }
        assert_eq!(serde_json::to_string(&Terrain::Grass).unwrap(), "\"grass\"");
    }
    #[test]
    fn everything_layable_has_a_price_and_rock_does_not() {
        for terrain in Terrain::LAYABLE {
            assert!(
                terrain.lay_cost().is_some_and(|cost| cost > 0),
                "{terrain:?} is free to lay"
            );
        }
        assert_eq!(Terrain::Rock.lay_cost(), None);
        assert!(
            !Terrain::LAYABLE.contains(&Terrain::Rock),
            "rock is on the toolbar and cannot be bought"
        );
    }

    #[test]
    fn a_path_costs_more_than_the_grass_it_replaces() {
        assert!(Terrain::Path.lay_cost() > Terrain::Grass.lay_cost());
    }

    #[test]
    fn everything_is_told_apart_by_name() {
        let all = [
            Terrain::Grass,
            Terrain::Path,
            Terrain::Dirt,
            Terrain::Water,
            Terrain::Rock,
        ];
        let mut names: Vec<_> = all.iter().map(|terrain| terrain.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), all.len());
    }
}
