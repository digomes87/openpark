//! What the park is trying to do, and whether it has managed it.
//!
//! A park without an objective is a toy: there is nothing it can fail at, so
//! nothing it does is a decision. One line decides the whole game — get this
//! many people through the gate at once, by this tick — and the only way to lose
//! it is to run out of money or to run out of time.
//!
//! The outcome is recorded rather than recomputed, because winning is a thing
//! that happens at a moment: a park that hit its target and then emptied out
//! still won.

use isogrid::time::Tick;
use serde::{Deserialize, Serialize};

use crate::park::Park;

/// What a park has been asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Objective {
    /// How many guests have to be in the park at once.
    pub guests: usize,
    /// What the park has to be rated, out of [`Rating::BEST`].
    pub rating: u32,
    /// The tick by which both have to be true.
    pub by: Tick,
}

impl Objective {
    /// How many guests a new park is asked for.
    ///
    /// Most of the way to [`Park::CAPACITY`].
    pub const GUESTS: usize = 100;

    /// What it is asked to be rated.
    ///
    /// A full park is not enough on its own: a field with a path across it fills
    /// up too, for a while, because people keep arriving faster than the bored
    /// ones leave. A rating has to be earned with rides worth queueing for,
    /// something to look at, and a crowd that is enjoying itself.
    pub const RATING: u32 = 600;

    /// How long it has to get them.
    ///
    /// Forty thousand ticks is about a quarter of an hour at
    /// [`isogrid::time::TickRate::CLASSIC`] — long enough to build something and
    /// let word of it get round, short enough that dithering loses.
    pub const DEADLINE: u64 = 40_000;

    /// What a new park is asked to do.
    pub const fn standard() -> Self {
        Self {
            guests: Self::GUESTS,
            rating: Self::RATING,
            by: Tick::new(Self::DEADLINE),
        }
    }

    /// What to call it in the HUD.
    pub fn describe(self) -> String {
        format!(
            "{} guests and a rating of {} by tick {}",
            self.guests,
            self.rating,
            self.by.get()
        )
    }
}

impl Default for Objective {
    fn default() -> Self {
        Self::standard()
    }
}

/// How the objective turned out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Still going.
    #[default]
    Pending,
    /// The park managed it.
    Won,
    /// It did not: the money ran out, or the time did.
    Lost,
}

impl Outcome {
    /// Whether the game is over, either way.
    pub const fn is_decided(self) -> bool {
        !matches!(self, Self::Pending)
    }

    /// What to call it in the HUD.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Pending => "in progress",
            Self::Won => "won",
            Self::Lost => "lost",
        }
    }
}

impl Park {
    /// What the park has been asked to do.
    pub const fn objective(&self) -> Objective {
        self.objective
    }

    /// Asks the park for something else instead.
    ///
    /// Whatever had been decided is undecided again: a new objective is a new
    /// game, and a park that had already won is now a park with something else
    /// to do.
    pub const fn ask_for(&mut self, objective: Objective) {
        self.objective = objective;
        self.outcome = Outcome::Pending;
    }

    /// How the objective is going.
    pub const fn outcome(&self) -> Outcome {
        self.outcome
    }

    /// Checks the objective, once.
    ///
    /// Called every tick. Bankruptcy loses immediately rather than at the
    /// deadline: a park the bank has closed is not going to fill up in the time
    /// it has left.
    pub(super) fn check_the_objective(&mut self) {
        if self.outcome.is_decided() {
            return;
        }

        if self.guests.len() >= self.objective.guests && self.reputation >= self.objective.rating {
            self.outcome = Outcome::Won;
            tracing::info!(
                guests = self.guests.len(),
                rating = self.reputation,
                tick = self.tick.get(),
                "the park met its objective"
            );
            return;
        }

        if self.is_bankrupt() || self.tick.get() >= self.objective.by.get() {
            self.outcome = Outcome::Lost;
            tracing::info!(
                guests = self.guests.len(),
                rating = self.reputation,
                wanted = self.objective.guests,
                bankrupt = self.is_bankrupt(),
                "the park missed its objective"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::park::fixtures::{bankrupt_park, run};
    use crate::park::{Campaign, Rating};

    #[test]
    fn a_full_park_is_not_enough_on_its_own() {
        // A field with a path across it fills up too, for a while. The rating
        // is the half of the objective that cannot be got by waiting.
        let mut park = Park::new("A Field", 32, 32, 5).unwrap();
        park.ask_for(Objective {
            guests: 1,
            rating: Rating::BEST,
            by: Tick::new(Park::SLOWEST_ARRIVALS * 40),
        });

        let park = run(park, Park::SLOWEST_ARRIVALS * 20);
        assert!(!park.guests().is_empty(), "nobody came at all");
        assert_eq!(
            park.outcome(),
            Outcome::Pending,
            "a field scored the best rating there is"
        );
    }

    #[test]
    fn a_new_park_has_something_to_do_and_has_not_done_it() {
        let park = Park::new("Forest Frontiers", 32, 32, 1).unwrap();

        assert_eq!(park.objective(), Objective::standard());
        assert_eq!(park.outcome(), Outcome::Pending);
        assert!(!park.outcome().is_decided());
        assert!(park.objective().describe().contains("100"));
    }

    #[test]
    fn filling_the_park_wins_it() {
        let mut park = Park::new("Popular", 32, 32, 5).unwrap();
        park.ask_for(Objective {
            guests: 3,
            rating: 0,
            by: Tick::new(Park::SLOWEST_ARRIVALS * 20),
        });

        let park = run(park, Park::SLOWEST_ARRIVALS * 10);
        assert_eq!(
            park.outcome(),
            Outcome::Won,
            "three guests was too much to ask"
        );
    }

    #[test]
    fn running_out_of_time_loses_it() {
        let mut park = Park::new("Quiet", 32, 32, 5).unwrap();
        park.ask_for(Objective {
            guests: Park::CAPACITY,
            rating: Objective::RATING,
            by: Tick::new(100),
        });

        let park = run(park, 200);
        assert_eq!(park.outcome(), Outcome::Lost);
    }

    #[test]
    fn running_out_of_money_loses_it_without_waiting_for_the_deadline() {
        let park = bankrupt_park();

        assert_eq!(park.outcome(), Outcome::Lost);
        assert!(
            park.tick().get() < park.objective().by.get(),
            "the deadline had already passed, so this proves nothing"
        );
    }

    #[test]
    fn winning_sticks_even_if_the_park_empties_afterwards() {
        let mut park = Park::new("Popular", 32, 32, 5).unwrap();
        park.ask_for(Objective {
            guests: 2,
            rating: 0,
            by: Tick::new(Park::SLOWEST_ARRIVALS * 40),
        });
        let mut park = run(park, Park::SLOWEST_ARRIVALS * 8);
        assert_eq!(park.outcome(), Outcome::Won);

        // Everybody out, and the deadline long past.
        for guest in &mut park.guests {
            guest.decide(crate::park::Plan::GoingHome);
        }
        let park = run(park, Park::SLOWEST_ARRIVALS * 60);

        assert_eq!(park.outcome(), Outcome::Won, "a won game came undone");
    }

    #[test]
    fn a_new_objective_is_a_new_game() {
        let mut park = Park::new("Popular", 32, 32, 5).unwrap();
        park.ask_for(Objective {
            guests: 1,
            rating: 0,
            by: Tick::new(Park::SLOWEST_ARRIVALS * 4),
        });
        let mut park = run(park, Park::SLOWEST_ARRIVALS * 3);
        assert_eq!(park.outcome(), Outcome::Won);

        park.ask_for(Objective {
            guests: Park::CAPACITY,
            rating: Objective::RATING,
            by: Tick::new(park.tick().get() + Campaign::LENGTH),
        });
        assert_eq!(park.outcome(), Outcome::Pending, "the old result stuck");
    }

    #[test]
    fn the_objective_survives_a_save() {
        let mut park = Park::new("Popular", 32, 32, 5).unwrap();
        park.ask_for(Objective {
            guests: 7,
            rating: 250,
            by: Tick::new(12_345),
        });
        let park = run(park, 500);

        let json = serde_json::to_string(&park).unwrap();
        let loaded: Park = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.objective(), park.objective());
        assert_eq!(loaded.outcome(), park.outcome());
    }
}
