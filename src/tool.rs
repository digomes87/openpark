//! What the mouse does when you click.

use crate::park::{Facility, FlatRide, Park, Scenery, Shop, StaffKind, Terrain, TrackPiece};

/// The thing the pointer is currently holding.
///
/// Cycled with the space bar and cancelled with escape, until the engine grows
/// number keys and a real toolbar can be bound to them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tool {
    /// Clicking does nothing. The default: a park is for looking at first.
    #[default]
    Inspect,
    /// Clicking puts one of these on the tile under the pointer.
    Build(Facility),
    /// Clicking takes down whatever is on the tile under the pointer.
    Demolish,
    /// Clicking puts the price of the shop under the pointer up a step.
    RaisePrice,
    /// Clicking brings it down a step.
    LowerPrice,
    /// Clicking takes somebody on. They start at the gate, not under the
    /// pointer: the park hires them, it does not place them.
    Hire(StaffKind),
    /// Clicking lets go of whoever is standing under the pointer.
    Fire,
    /// Clicking raises the land under the pointer by a step.
    Raise,
    /// Clicking digs it down by a step.
    Lower,
    /// Clicking lays a new surface on the tile under the pointer.
    Lay(Terrain),
    /// Clicking starts a new ride, with its first piece of track to go on the
    /// tile under the pointer.
    StartRide,
    /// Clicking lays one more piece on the ride being built.
    Track(TrackPiece),
    /// Clicking takes the last piece of track back off it.
    Unlay,
    /// Clicking sends a test train round the ride under the pointer.
    TestRide,
    /// Clicking opens it to the queue.
    OpenRide,
    /// Clicking shuts it.
    CloseRide,
    /// Clicking takes the whole thing down.
    DemolishRide,
    /// Clicking writes the park to its save file.
    Save,
    /// Clicking reads it back, throwing away everything since.
    Load,
    /// Clicking puts up something to look at.
    Plant(Scenery),
    /// Clicking takes it down again.
    Uproot,
    /// Clicking buys one of these and stands it on the tile under the pointer.
    Buy(FlatRide),
}

impl Tool {
    /// The toolbars, one per number key.
    ///
    /// The number row picks a toolbar and the space bar walks along the one in
    /// hand. That is the only arrangement that stays usable as the list grows:
    /// reaching "curve left" by pressing space twenty times is not a toolbar,
    /// which is why the engine grew the number row.
    pub const KITS: [&'static [Self]; 10] = [
        // 1: look at things.
        &[Self::Inspect],
        // 2: stalls and benches.
        &[
            Self::Build(Facility::FoodStall),
            Self::Build(Facility::Bench),
            Self::Demolish,
        ],
        // 3: what anything charges, shop or ride.
        &[Self::RaisePrice, Self::LowerPrice],
        // 4: the payroll.
        &[
            Self::Hire(StaffKind::Handyman),
            Self::Hire(StaffKind::Entertainer),
            Self::Hire(StaffKind::Mechanic),
            Self::Fire,
        ],
        // 5: the land itself.
        &[
            Self::Raise,
            Self::Lower,
            Self::Lay(Terrain::Path),
            Self::Lay(Terrain::Queue),
            Self::Lay(Terrain::Grass),
            Self::Lay(Terrain::Dirt),
            Self::Lay(Terrain::Water),
        ],
        // 6: laying track.
        &[
            Self::StartRide,
            Self::Track(TrackPiece::Station),
            Self::Track(TrackPiece::Straight),
            Self::Track(TrackPiece::CurveLeft),
            Self::Track(TrackPiece::CurveRight),
            Self::Track(TrackPiece::SlopeUp),
            Self::Track(TrackPiece::SlopeDown),
            Self::Track(TrackPiece::LiftHill),
            Self::Track(TrackPiece::Brakes),
            Self::Track(TrackPiece::Powered),
            Self::Unlay,
        ],
        // 7: running the rides.
        &[
            Self::TestRide,
            Self::OpenRide,
            Self::CloseRide,
            Self::DemolishRide,
        ],
        // 8: the save file. A click rather than a key, because the engine's
        // keyboard is the number row, the arrows, space and escape — and all of
        // those are spoken for.
        &[Self::Save, Self::Load],
        // 9: things to look at.
        &[
            Self::Plant(Scenery::Tree),
            Self::Plant(Scenery::Flowerbed),
            Self::Plant(Scenery::Fountain),
            Self::Plant(Scenery::Lamp),
            Self::Uproot,
        ],
        // 0: rides that come as they are. Last on the row and last in the list,
        // because the number row runs 1 to 9 and then round to 0.
        &[
            Self::Buy(FlatRide::Carousel),
            Self::Buy(FlatRide::FerrisWheel),
            Self::Buy(FlatRide::HauntedHouse),
            Self::Buy(FlatRide::TeaCups),
        ],
    ];

    /// Which toolbar this tool is on, counting from zero.
    pub fn kit(self) -> usize {
        Self::KITS
            .iter()
            .position(|kit| kit.contains(&self))
            .unwrap_or(0)
    }

    /// The first tool on toolbar `kit`, or `None` if there is no such toolbar.
    ///
    /// ```
    /// # use openpark::tool::Tool;
    /// assert_eq!(Tool::from_kit(0), Some(Tool::Inspect));
    /// assert_eq!(Tool::from_kit(99), None, "there is no hundredth toolbar");
    /// ```
    pub fn from_kit(kit: usize) -> Option<Self> {
        Self::KITS.get(kit)?.first().copied()
    }

    /// The next tool along the toolbar in hand, wrapping round it.
    ///
    /// ```
    /// # use openpark::park::Facility;
    /// # use openpark::tool::Tool;
    /// let stall = Tool::Build(Facility::FoodStall);
    ///
    /// let mut tool = stall;
    /// for _ in 0..Tool::KITS[stall.kit()].len() {
    ///     tool = tool.next();
    /// }
    /// assert_eq!(tool, stall, "the cycle should come back round");
    /// ```
    #[must_use]
    pub fn next(self) -> Self {
        let kit = Self::KITS[self.kit()];
        let at = kit.iter().position(|tool| *tool == self).unwrap_or(0);
        kit[(at + 1) % kit.len()]
    }

    /// What flat ride it is buying, if it is buying one.
    pub const fn flat_ride(self) -> Option<FlatRide> {
        match self {
            Self::Buy(kind) => Some(kind),
            _ => None,
        }
    }

    /// What it is planting, if it is planting anything.
    pub const fn scenery(self) -> Option<Scenery> {
        match self {
            Self::Plant(scenery) => Some(scenery),
            _ => None,
        }
    }

    /// What piece of track it is laying, if it is laying one.
    pub const fn track(self) -> Option<TrackPiece> {
        match self {
            Self::Track(piece) => Some(piece),
            _ => None,
        }
    }

    /// Whether this tool wants a heading, which the arrow keys set while it is
    /// in hand.
    pub const fn wants_a_heading(self) -> bool {
        matches!(self, Self::StartRide)
    }

    /// What it is building, if it is building anything.
    pub const fn facility(self) -> Option<Facility> {
        match self {
            Self::Build(facility) => Some(facility),
            _ => None,
        }
    }

    /// What surface it is laying, if it is laying one.
    pub const fn terrain(self) -> Option<Terrain> {
        match self {
            Self::Lay(terrain) => Some(terrain),
            _ => None,
        }
    }

    /// Who it is hiring, if it is hiring anybody.
    pub const fn staff(self) -> Option<StaffKind> {
        match self {
            Self::Hire(kind) => Some(kind),
            _ => None,
        }
    }

    /// Whether clicking with this tool changes the park.
    pub const fn is_active(self) -> bool {
        !matches!(self, Self::Inspect)
    }

    /// What the HUD calls it.
    pub fn label(self) -> String {
        match self {
            Self::Inspect => "Look around".to_owned(),
            Self::Build(facility) => {
                format!("Build {} ({})", facility.name(), facility.build_cost())
            }
            Self::Demolish => "Demolish".to_owned(),
            Self::RaisePrice => format!("Put the price up (+{})", Shop::PRICE_STEP),
            Self::LowerPrice => format!("Bring the price down (-{})", Shop::PRICE_STEP),
            Self::Hire(kind) => format!("Hire {} ({})", kind.name(), kind.hire_cost()),
            Self::Fire => "Let somebody go".to_owned(),
            Self::Raise => format!("Raise the land ({})", Park::LANDSCAPING),
            Self::Lower => format!("Dig the land down ({})", Park::LANDSCAPING),
            Self::Lay(terrain) => terrain.lay_cost().map_or_else(
                || format!("Lay {}", terrain.name().to_lowercase()),
                |cost| format!("Lay {} ({cost})", terrain.name().to_lowercase()),
            ),
            Self::StartRide => "Start a new ride".to_owned(),
            Self::Track(piece) => format!("Lay {} ({})", piece.name().to_lowercase(), piece.cost()),
            Self::Unlay => "Take the last piece of track off".to_owned(),
            Self::TestRide => "Test the ride".to_owned(),
            Self::OpenRide => "Open the ride".to_owned(),
            Self::CloseRide => "Shut the ride".to_owned(),
            Self::DemolishRide => "Demolish the ride".to_owned(),
            Self::Save => format!("Save the park to {}", crate::save::DEFAULT_PATH),
            Self::Load => format!("Load {}", crate::save::DEFAULT_PATH),
            Self::Plant(scenery) => {
                format!(
                    "Plant {} ({})",
                    scenery.name().to_lowercase(),
                    scenery.cost()
                )
            }
            Self::Uproot => "Take the scenery down".to_owned(),
            Self::Buy(kind) => format!(
                "Buy a {} ({}, {} by {})",
                kind.name().to_lowercase(),
                kind.cost(),
                kind.footprint(),
                kind.footprint()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_game_is_only_looking() {
        assert_eq!(Tool::default(), Tool::Inspect);
        assert!(!Tool::default().is_active());
    }

    /// Every tool in the game, across every toolbar.
    fn all_tools() -> Vec<Tool> {
        Tool::KITS
            .iter()
            .flat_map(|kit| kit.iter().copied())
            .collect()
    }

    #[test]
    fn every_toolbar_cycles_through_itself_and_comes_back() {
        for (number, kit) in Tool::KITS.iter().enumerate() {
            let first = Tool::from_kit(number).expect("every toolbar has a tool on it");
            let mut seen = Vec::new();
            let mut tool = first;

            for _ in 0..kit.len() {
                seen.push(tool);
                tool = tool.next();
            }

            assert_eq!(tool, first, "toolbar {number} does not come back round");
            for expected in *kit {
                assert!(seen.contains(expected), "{expected:?} is unreachable");
            }
        }
    }

    #[test]
    fn a_tool_only_ever_cycles_within_its_own_toolbar() {
        for tool in all_tools() {
            assert_eq!(
                tool.next().kit(),
                tool.kit(),
                "{tool:?} cycles off its own toolbar"
            );
        }
    }

    #[test]
    fn no_tool_is_on_two_toolbars() {
        let mut every = all_tools();
        let laid_out = every.len();
        every.sort_by_key(|tool| format!("{tool:?}"));
        every.dedup();

        assert_eq!(every.len(), laid_out, "a tool appears on two toolbars");
    }

    #[test]
    fn everything_that_is_not_inspecting_changes_the_park() {
        for tool in all_tools() {
            assert_eq!(tool.is_active(), tool != Tool::Inspect, "{tool:?}");
        }
    }

    #[test]
    fn only_a_building_tool_is_building_something() {
        assert_eq!(Tool::Inspect.facility(), None);
        assert_eq!(Tool::Demolish.facility(), None);
        assert_eq!(Tool::RaisePrice.facility(), None);
        assert_eq!(
            Tool::Build(Facility::Bench).facility(),
            Some(Facility::Bench)
        );
    }

    #[test]
    fn only_a_hiring_tool_is_hiring_somebody() {
        assert_eq!(Tool::Inspect.staff(), None);
        assert_eq!(Tool::Fire.staff(), None);
        assert_eq!(
            Tool::Hire(StaffKind::Handyman).staff(),
            Some(StaffKind::Handyman)
        );
    }

    #[test]
    fn only_a_laying_tool_is_laying_something() {
        assert_eq!(Tool::Inspect.terrain(), None);
        assert_eq!(Tool::Raise.terrain(), None);
        assert_eq!(Tool::Lay(Terrain::Path).terrain(), Some(Terrain::Path));
    }

    #[test]
    fn every_flat_ride_is_in_the_cycle() {
        for kind in FlatRide::ALL {
            assert!(
                all_tools().contains(&Tool::Buy(kind)),
                "{kind:?} cannot be bought with the mouse"
            );
        }
    }

    #[test]
    fn everything_that_can_be_planted_is_in_the_cycle() {
        for scenery in Scenery::ALL {
            assert!(
                all_tools().contains(&Tool::Plant(scenery)),
                "{scenery:?} cannot be planted with the mouse"
            );
        }
    }

    #[test]
    fn everything_that_can_be_laid_is_in_the_cycle() {
        for terrain in Terrain::LAYABLE {
            assert!(
                all_tools().contains(&Tool::Lay(terrain)),
                "{terrain:?} cannot be laid with the mouse"
            );
        }
    }

    #[test]
    fn everything_that_can_be_built_or_hired_is_in_the_cycle() {
        for facility in Facility::ALL {
            assert!(
                all_tools().contains(&Tool::Build(facility)),
                "{facility:?} cannot be built with the mouse"
            );
        }
        for kind in StaffKind::ALL {
            assert!(
                all_tools().contains(&Tool::Hire(kind)),
                "{kind:?} cannot be hired with the mouse"
            );
        }
    }

    #[test]
    fn every_tool_says_what_it_is_and_what_it_costs() {
        for tool in all_tools() {
            let label = tool.label();
            assert!(!label.is_empty(), "{tool:?} has no label");

            if let Some(facility) = tool.facility() {
                assert!(label.contains(facility.name()));
                assert!(label.contains(&facility.build_cost().to_string()));
            }

            if let Some(kind) = tool.staff() {
                assert!(label.contains(kind.name()));
                assert!(label.contains(&kind.hire_cost().to_string()));
            }

            if let Some(terrain) = tool.terrain() {
                assert!(label
                    .to_lowercase()
                    .contains(&terrain.name().to_lowercase()));
            }
        }
    }
}
