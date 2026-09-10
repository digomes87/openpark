//! What a guest wants.
//!
//! Three numbers between 0 and 1, and the rules by which one tick of being in
//! the park changes them. Deliberately free of any mention of the park itself:
//! nothing here knows what a tile is, which is what makes the balance testable
//! by running the numbers rather than the game.

use serde::{Deserialize, Serialize};

/// How a guest is feeling.
///
/// Hunger climbs, energy drains, and happiness follows from the two of them —
/// a guest who is fed and rested slowly cheers up, and one who is neither slowly
/// gives up and goes home.
///
/// ```
/// # use openpark::park::Needs;
/// let mut needs = Needs::fresh();
/// assert!(needs.happiness() > 0.5, "guests arrive in a good mood");
///
/// for _ in 0..Needs::TICKS_UNTIL_HUNGRY {
///     needs.wear_down(true);
/// }
/// assert!(needs.hunger() > 0.5, "a walk should work up an appetite");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Needs {
    hunger: f32,
    energy: f32,
    happiness: f32,
}

impl Needs {
    /// How hungry a guest is, from 0 (just eaten) to 1 (starving).
    pub const fn hunger(self) -> f32 {
        self.hunger
    }

    /// How much walking a guest has left in it, from 1 (fresh) to 0 (footsore).
    pub const fn energy(self) -> f32 {
        self.energy
    }

    /// How the visit is going, from 1 (delighted) to 0 (miserable).
    pub const fn happiness(self) -> f32 {
        self.happiness
    }

    /// How many ticks of walking it takes to go from fed to starving.
    ///
    /// Five minutes at [`isogrid::time::TickRate::CLASSIC`]. Long enough that a
    /// guest sees something of the park first, short enough that the lack of
    /// anywhere to eat is felt within one sitting.
    pub const TICKS_UNTIL_HUNGRY: u32 = 12_000;

    /// How many ticks of walking it takes to wear a guest out completely.
    const TICKS_UNTIL_EXHAUSTED: u32 = 16_000;

    /// How many ticks a guest with everything it needs takes to cheer up fully.
    const TICKS_UNTIL_DELIGHTED: u32 = 20_000;

    /// How many ticks of going without turn a delighted guest into a miserable
    /// one.
    const TICKS_UNTIL_MISERABLE: u32 = 6_000;

    /// Standing still is restful: needs move at this fraction of their usual
    /// rate while a guest is not walking anywhere.
    const RESTING: f32 = 0.25;

    /// The hunger past which a guest starts to resent being here.
    const TOO_HUNGRY: f32 = 0.6;

    /// The energy below which the same happens.
    const TOO_TIRED: f32 = 0.3;

    /// The hunger at which a guest starts looking for something to eat.
    ///
    /// Deliberately below [`Needs::TOO_HUNGRY`]: a guest that only went looking
    /// once it was miserable would never find a stall in time.
    const WANTS_FOOD: f32 = 0.35;

    /// The energy at which a guest starts looking for somewhere to sit.
    const WANTS_A_SIT_DOWN: f32 = 0.5;

    /// How much better a guest feels for having what it wanted.
    const SATISFACTION: f32 = 0.15;

    /// The happiness at which a guest gives up on the day and heads home.
    const FED_UP: f32 = 0.3;

    /// How a guest feels on the way through the gate: rested, fed, and looking
    /// forward to it.
    pub const fn fresh() -> Self {
        Self {
            hunger: 0.0,
            energy: 1.0,
            happiness: 0.8,
        }
    }

    /// One tick of being alive in the park.
    ///
    /// `walking` slows everything down when false, so a guest waiting somewhere
    /// wears out more slowly than one tramping across the grass.
    pub fn wear_down(&mut self, walking: bool) {
        let effort = if walking { 1.0 } else { Self::RESTING };

        self.hunger = (self.hunger + Self::rate(Self::TICKS_UNTIL_HUNGRY) * effort).min(1.0);
        self.energy = (self.energy - Self::rate(Self::TICKS_UNTIL_EXHAUSTED) * effort).max(0.0);

        let mood = if self.is_suffering() {
            -Self::rate(Self::TICKS_UNTIL_MISERABLE)
        } else {
            Self::rate(Self::TICKS_UNTIL_DELIGHTED)
        };
        self.happiness = (self.happiness + mood).clamp(0.0, 1.0);
    }

    /// Whether the guest would like something to eat.
    ///
    /// ```
    /// # use openpark::park::Needs;
    /// let mut needs = Needs::fresh();
    /// assert!(!needs.wants_food(), "nobody arrives hungry");
    ///
    /// for _ in 0..Needs::TICKS_UNTIL_HUNGRY / 2 {
    ///     needs.wear_down(true);
    /// }
    /// assert!(needs.wants_food());
    ///
    /// needs.eat();
    /// assert!(!needs.wants_food());
    /// ```
    pub fn wants_food(self) -> bool {
        self.hunger > Self::WANTS_FOOD
    }

    /// Whether the guest would like to sit down.
    pub fn wants_a_sit_down(self) -> bool {
        self.energy < Self::WANTS_A_SIT_DOWN
    }

    /// A meal: no longer hungry, and pleased about it.
    pub fn eat(&mut self) {
        self.hunger = 0.0;
        self.cheer_up();
    }

    /// A sit down: back on its feet, and pleased about it.
    pub fn rest(&mut self) {
        self.energy = 1.0;
        self.cheer_up();
    }

    /// The lift from getting what you wanted.
    fn cheer_up(&mut self) {
        self.happiness = (self.happiness + Self::SATISFACTION).min(1.0);
    }

    /// Whether the guest wants something it cannot have — which, until there
    /// are shops and benches, is the only way a mood goes down.
    pub fn is_suffering(self) -> bool {
        self.hunger > Self::TOO_HUNGRY || self.energy < Self::TOO_TIRED
    }

    /// Whether the guest has had enough and wants to go home.
    ///
    /// ```
    /// # use openpark::park::Needs;
    /// let mut needs = Needs::fresh();
    /// assert!(!needs.is_fed_up(), "nobody leaves the moment they arrive");
    ///
    /// for _ in 0..100_000 {
    ///     needs.wear_down(true);
    /// }
    /// assert!(needs.is_fed_up(), "a park with nothing in it empties out");
    /// ```
    pub fn is_fed_up(self) -> bool {
        self.happiness < Self::FED_UP
    }

    /// How much a need moves in one tick, given how many ticks it takes to
    /// cross its whole range.
    fn rate(ticks: u32) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let ticks = ticks as f32;
        1.0 / ticks
    }
}

impl Default for Needs {
    fn default() -> Self {
        Self::fresh()
    }
}

#[cfg(test)]
mod tests {
    // The comparisons below are all against the exact ends of a need's range,
    // which are set by a clamp rather than arrived at by arithmetic.
    #![allow(clippy::float_cmp)]

    use super::*;

    /// Wears a fresh set of needs down for `ticks` ticks of walking.
    fn after(ticks: u32) -> Needs {
        let mut needs = Needs::fresh();
        for _ in 0..ticks {
            needs.wear_down(true);
        }
        needs
    }

    #[test]
    fn a_guest_arrives_fed_rested_and_cheerful() {
        let needs = Needs::fresh();
        assert_eq!(needs.hunger(), 0.0);
        assert_eq!(needs.energy(), 1.0);
        assert!(needs.happiness() > 0.5);
        assert!(!needs.is_suffering());
        assert!(!needs.is_fed_up());
    }

    #[test]
    fn walking_makes_a_guest_hungry_and_tired() {
        let needs = after(1_000);
        assert!(needs.hunger() > Needs::fresh().hunger());
        assert!(needs.energy() < Needs::fresh().energy());
    }

    #[test]
    fn resting_wears_a_guest_down_more_slowly_than_walking() {
        let mut walked = Needs::fresh();
        let mut rested = Needs::fresh();
        for _ in 0..1_000 {
            walked.wear_down(true);
            rested.wear_down(false);
        }

        assert!(rested.hunger() < walked.hunger());
        assert!(rested.energy() > walked.energy());
    }

    #[test]
    fn a_guest_with_nothing_wrong_cheers_up_to_a_point() {
        let needs = after(100);
        assert!(needs.happiness() > Needs::fresh().happiness());

        let mut delighted = Needs::fresh();
        for _ in 0..Needs::TICKS_UNTIL_DELIGHTED {
            // Kept fed and rested by hand: the mood should stop at the top.
            delighted.wear_down(false);
            delighted = Needs {
                hunger: 0.0,
                energy: 1.0,
                ..delighted
            };
        }
        assert_eq!(delighted.happiness(), 1.0);
    }

    #[test]
    fn hunger_and_tiredness_are_what_spoil_the_day() {
        let content = after(1_000);
        assert!(!content.is_suffering());

        let starving = Needs {
            hunger: 1.0,
            ..Needs::fresh()
        };
        assert!(starving.is_suffering());

        let footsore = Needs {
            energy: 0.0,
            ..Needs::fresh()
        };
        assert!(footsore.is_suffering());
    }

    #[test]
    fn a_park_with_nothing_to_do_eventually_sends_a_guest_home() {
        let mut needs = Needs::fresh();
        let mut ticks = 0;
        while !needs.is_fed_up() {
            needs.wear_down(true);
            ticks += 1;
            assert!(ticks < 200_000, "this guest is never going to leave");
        }

        // Nobody storms out before there is something to be unhappy about: the
        // first complaint is hunger, which takes a while to build.
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let first_complaint = (Needs::TICKS_UNTIL_HUNGRY as f32 * Needs::TOO_HUNGRY) as u32;
        assert!(
            ticks > first_complaint,
            "a guest gave up after {ticks} ticks, before it had a reason to"
        );
    }

    #[test]
    fn no_need_ever_leaves_its_range() {
        let mut needs = Needs::fresh();
        for tick in 0..200_000 {
            needs.wear_down(tick % 2 == 0);
            for (name, value) in [
                ("hunger", needs.hunger()),
                ("energy", needs.energy()),
                ("happiness", needs.happiness()),
            ] {
                assert!(
                    (0.0..=1.0).contains(&value),
                    "{name} reached {value} after {tick} ticks"
                );
            }
        }
    }

    #[test]
    fn a_guest_goes_looking_before_it_gets_desperate() {
        let mut needs = Needs::fresh();
        let mut looked_for_food = None;
        let mut suffered = None;

        for tick in 0..Needs::TICKS_UNTIL_HUNGRY {
            needs.wear_down(true);
            if looked_for_food.is_none() && needs.wants_food() {
                looked_for_food = Some(tick);
            }
            if suffered.is_none() && needs.is_suffering() {
                suffered = Some(tick);
            }
        }

        assert!(
            looked_for_food.unwrap() < suffered.unwrap(),
            "a guest only went looking for food once it was already miserable"
        );
    }

    #[test]
    fn eating_settles_hunger_and_lifts_the_mood() {
        let mut needs = after(Needs::TICKS_UNTIL_HUNGRY);
        let mood = needs.happiness();

        needs.eat();
        assert_eq!(needs.hunger(), 0.0);
        assert!(needs.happiness() > mood);
    }

    #[test]
    fn sitting_down_restores_the_feet_and_lifts_the_mood() {
        let mut needs = after(Needs::TICKS_UNTIL_HUNGRY);
        let mood = needs.happiness();

        needs.rest();
        assert_eq!(needs.energy(), 1.0);
        assert!(needs.happiness() > mood);
    }

    #[test]
    fn being_pleased_never_takes_a_guest_past_delighted() {
        let mut needs = Needs::fresh();
        for _ in 0..100 {
            needs.eat();
            needs.rest();
        }
        assert_eq!(needs.happiness(), 1.0);
    }

    #[test]
    fn a_park_that_feeds_its_guests_keeps_them() {
        let mut needs = Needs::fresh();
        for _ in 0..200_000 {
            needs.wear_down(true);
            if needs.wants_food() {
                needs.eat();
            }
            if needs.wants_a_sit_down() {
                needs.rest();
            }
            assert!(!needs.is_fed_up(), "a well-served guest gave up anyway");
        }
    }

    #[test]
    fn needs_survive_a_save() {
        let needs = after(500);
        let json = serde_json::to_string(&needs).unwrap();
        assert_eq!(serde_json::from_str::<Needs>(&json).unwrap(), needs);
    }
}
