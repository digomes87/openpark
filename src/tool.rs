//! What the mouse does when you click.

use crate::park::{Facility, Shop, StaffKind};

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
}

impl Tool {
    /// Every tool, in the order the space bar walks through them.
    pub const ORDER: [Self; 9] = [
        Self::Inspect,
        Self::Build(Facility::FoodStall),
        Self::Build(Facility::Bench),
        Self::Demolish,
        Self::RaisePrice,
        Self::LowerPrice,
        Self::Hire(StaffKind::Handyman),
        Self::Hire(StaffKind::Entertainer),
        Self::Fire,
    ];

    /// The next tool along, wrapping back to [`Tool::Inspect`].
    ///
    /// ```
    /// # use openpark::tool::Tool;
    /// let mut tool = Tool::Inspect;
    /// for _ in 0..Tool::ORDER.len() {
    ///     tool = tool.next();
    /// }
    /// assert_eq!(tool, Tool::Inspect, "the cycle should come back round");
    /// ```
    #[must_use]
    pub fn next(self) -> Self {
        let at = Self::ORDER.iter().position(|tool| *tool == self);
        Self::ORDER[(at.unwrap_or(0) + 1) % Self::ORDER.len()]
    }

    /// What it is building, if it is building anything.
    pub const fn facility(self) -> Option<Facility> {
        match self {
            Self::Build(facility) => Some(facility),
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

    #[test]
    fn the_cycle_reaches_everything_and_comes_back() {
        let mut seen = Vec::new();
        let mut tool = Tool::Inspect;
        for _ in 0..Tool::ORDER.len() {
            seen.push(tool);
            tool = tool.next();
        }

        assert_eq!(tool, Tool::Inspect);
        for expected in Tool::ORDER {
            assert!(seen.contains(&expected), "{expected:?} is unreachable");
        }
    }

    #[test]
    fn everything_that_is_not_inspecting_changes_the_park() {
        for tool in Tool::ORDER {
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
    fn everything_that_can_be_built_or_hired_is_in_the_cycle() {
        for facility in Facility::ALL {
            assert!(
                Tool::ORDER.contains(&Tool::Build(facility)),
                "{facility:?} cannot be built with the mouse"
            );
        }
        for kind in StaffKind::ALL {
            assert!(
                Tool::ORDER.contains(&Tool::Hire(kind)),
                "{kind:?} cannot be hired with the mouse"
            );
        }
    }

    #[test]
    fn every_tool_says_what_it_is_and_what_it_costs() {
        for tool in Tool::ORDER {
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
        }
    }
}
