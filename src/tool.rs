//! What the mouse does when you click.

use crate::park::Facility;

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
}

impl Tool {
    /// Every tool, in the order the space bar walks through them.
    pub const ORDER: [Self; 4] = [
        Self::Inspect,
        Self::Build(Facility::FoodStall),
        Self::Build(Facility::Bench),
        Self::Demolish,
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
            Self::Inspect | Self::Demolish => None,
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
        assert_eq!(
            Tool::Build(Facility::Bench).facility(),
            Some(Facility::Bench)
        );
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
        }
    }
}
