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
    /// Bare earth, usually where something was demolished.
    Dirt,
    /// Water. Not walkable, not buildable without work.
    Water,
    /// Solid rock. Not walkable, not buildable.
    Rock,
}

impl Terrain {
    /// Whether a guest can stand here.
    ///
    /// ```
    /// # use openpark::park::Terrain;
    /// assert!(Terrain::Path.is_walkable());
    /// assert!(!Terrain::Water.is_walkable());
    /// ```
    pub const fn is_walkable(self) -> bool {
        matches!(self, Self::Grass | Self::Path | Self::Dirt)
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
            Self::Path => Some(1),
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
}
