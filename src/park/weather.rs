//! What the sky is doing, and what that does to everybody under it.
//!
//! Weather is the one thing in the park nobody chose. It changes on its own from
//! the park's own dice, so the same seed gets the same afternoon, and it pulls
//! on almost everything else: how thirsty the crowd gets, how many people turn
//! up at all, and how much anybody is enjoying themselves.
//!
//! The point of it is that a good park is not one arrangement of tiles. A day of
//! sun sells drinks and fills the gate; a wet afternoon empties both, and the
//! park that stocked up for the sun is the one holding the stock.

use serde::{Deserialize, Serialize};

use crate::park::Park;

/// What the sky is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Weather {
    /// Hot and bright. Thirsty work, and everybody comes out.
    #[default]
    Sunny,
    /// Grey and dry. The ordinary sort of day.
    Cloudy,
    /// Rain. Fewer people, and the ones already here wish they were not.
    Rain,
    /// A downpour. Rain, but nobody is pretending it is fine.
    Storm,
}

impl Weather {
    /// Every kind of day, from the best to the worst.
    pub const ALL: [Self; 4] = [Self::Sunny, Self::Cloudy, Self::Rain, Self::Storm];

    /// How long one spell of weather lasts, in ticks.
    ///
    /// Two wage bills: long enough to be worth reacting to, short enough that a
    /// wet spell is a setback rather than the end of the park.
    pub const SPELL: u64 = 2_400;

    /// Whether anybody is getting wet.
    pub const fn is_wet(self) -> bool {
        matches!(self, Self::Rain | Self::Storm)
    }

    /// How much faster the crowd gets thirsty than on a grey day.
    ///
    /// Sun is the reason to own a drink stall; rain is the reason not to own
    /// only drink stalls.
    pub const fn thirst(self) -> f32 {
        match self {
            Self::Sunny => 1.8,
            Self::Cloudy => 1.0,
            Self::Rain => 0.6,
            Self::Storm => 0.4,
        }
    }

    /// How many of the people who would have come actually turn up, from 0 to 1.
    pub const fn turnout(self) -> f32 {
        match self {
            Self::Sunny => 1.0,
            Self::Cloudy => 0.85,
            Self::Rain => 0.5,
            Self::Storm => 0.25,
        }
    }

    /// How much mood one tick of this takes out of somebody standing in it.
    ///
    /// Nothing at all in the dry: a sunny day is not a treat that wears off, it
    /// is simply not a problem.
    pub fn misery(self) -> f32 {
        match self {
            Self::Sunny | Self::Cloudy => 0.0,
            Self::Rain => 1.0 / 9_000.0,
            Self::Storm => 1.0 / 4_000.0,
        }
    }

    /// What to call it in the HUD.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sunny => "Sunny",
            Self::Cloudy => "Cloudy",
            Self::Rain => "Raining",
            Self::Storm => "Stormy",
        }
    }

    /// What the sky is drawn as behind the park.
    pub const fn sky(self) -> isogrid::render::Color {
        match self {
            Self::Sunny => isogrid::render::Color::hex(0x1B_26_33),
            Self::Cloudy => isogrid::render::Color::hex(0x25_2A_31),
            Self::Rain => isogrid::render::Color::hex(0x1E_22_28),
            Self::Storm => isogrid::render::Color::hex(0x12_14_18),
        }
    }

    /// How much the daylight is dimmed, as a factor on every colour drawn.
    pub const fn daylight(self) -> f32 {
        match self {
            Self::Sunny => 1.0,
            Self::Cloudy => 0.92,
            Self::Rain => 0.82,
            Self::Storm => 0.7,
        }
    }

    /// What the weather does next, given a roll of `chance` from 0 to 1.
    ///
    /// A walk rather than a shuffle: sun goes cloudy before it rains, and a
    /// storm blows itself out through rain. An afternoon therefore reads as
    /// weather rather than as a slideshow.
    ///
    /// ```
    /// # use openpark::park::Weather;
    /// assert_eq!(Weather::Sunny.next(0.0), Weather::Sunny, "a fine day holds");
    /// assert_eq!(Weather::Sunny.next(0.99), Weather::Cloudy, "and then clouds over");
    /// assert_ne!(Weather::Storm.next(0.99), Weather::Sunny, "a storm does not vanish");
    /// ```
    #[must_use]
    pub fn next(self, chance: f32) -> Self {
        let (worse, better) = match self {
            Self::Sunny => (Self::Cloudy, Self::Sunny),
            Self::Cloudy => (Self::Rain, Self::Sunny),
            Self::Rain => (Self::Storm, Self::Cloudy),
            Self::Storm => (Self::Storm, Self::Rain),
        };

        // Half the time nothing changes, and the rest is split between the step
        // up and the step down, which leaves most of an afternoon dry.
        if chance > 0.75 {
            worse
        } else if chance > 0.5 {
            better
        } else {
            self
        }
    }
}

impl Park {
    /// What the sky is doing.
    pub const fn weather(&self) -> Weather {
        self.weather
    }

    /// Puts the weather where you want it.
    ///
    /// For a park being set up deliberately, and for tests that are about the
    /// rain rather than about waiting for it.
    pub const fn set_weather(&mut self, weather: Weather) {
        self.weather = weather;
    }

    /// Moves the weather on, once a spell has run out.
    pub(super) fn watch_the_sky(&mut self) {
        if !self.tick.is_multiple_of(Weather::SPELL) {
            return;
        }

        let was = self.weather;
        self.weather = self.weather.next(self.rng.next_f32());

        if self.weather != was {
            tracing::debug!(
                from = was.name(),
                to = self.weather.name(),
                "the weather turned"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    // The misery of a dry day is zero because it is written down as zero, not
    // because anything worked it out.
    #![allow(clippy::float_cmp)]

    use super::*;

    use crate::park::fixtures::run;
    use crate::park::Thought;

    #[test]
    fn a_park_opens_on_a_fine_day() {
        let park = Park::new("Forest Frontiers", 32, 32, 1).unwrap();
        assert_eq!(park.weather(), Weather::Sunny);
        assert!(!park.weather().is_wet());
    }

    #[test]
    fn every_kind_of_day_is_worse_than_the_one_before_it() {
        for pair in Weather::ALL.windows(2) {
            let (better, worse) = (pair[0], pair[1]);

            assert!(
                better.turnout() > worse.turnout(),
                "{better:?} brings no more people than {worse:?}"
            );
            assert!(
                better.misery() <= worse.misery(),
                "{better:?} is no nicer to stand in than {worse:?}"
            );
            assert!(
                better.daylight() > worse.daylight(),
                "{better:?} is no brighter than {worse:?}"
            );
        }
    }

    #[test]
    fn the_dry_days_are_the_ones_nobody_gets_wet_on() {
        assert!(!Weather::Sunny.is_wet());
        assert!(!Weather::Cloudy.is_wet());
        assert!(Weather::Rain.is_wet());
        assert!(Weather::Storm.is_wet());

        for dry in [Weather::Sunny, Weather::Cloudy] {
            assert_eq!(dry.misery(), 0.0, "{dry:?} makes people miserable");
        }
    }

    #[test]
    fn sun_is_thirsty_work_and_rain_is_not() {
        assert!(Weather::Sunny.thirst() > Weather::Cloudy.thirst());
        assert!(Weather::Cloudy.thirst() > Weather::Rain.thirst());
        assert!(Weather::Storm.thirst() < 1.0);
    }

    #[test]
    fn the_weather_walks_rather_than_jumping() {
        // Whatever the roll, tomorrow is next door to today.
        for today in Weather::ALL {
            for roll in [0.0, 0.4, 0.6, 0.9, 1.0] {
                let tomorrow = today.next(roll);
                let steps = |weather: Weather| {
                    Weather::ALL
                        .iter()
                        .position(|kind| *kind == weather)
                        .expect("every kind is in ALL")
                };

                let moved = steps(today).abs_diff(steps(tomorrow));
                assert!(
                    moved <= 1,
                    "{today:?} became {tomorrow:?} on a roll of {roll}"
                );
            }
        }
    }

    #[test]
    fn the_weather_turns_on_its_own_and_the_same_seed_gets_the_same_day() {
        let one = run(
            Park::new("Weather", 32, 32, 4).unwrap(),
            Weather::SPELL * 12,
        );
        let two = run(
            Park::new("Weather", 32, 32, 4).unwrap(),
            Weather::SPELL * 12,
        );
        assert_eq!(one.weather(), two.weather());

        // Over a long enough afternoon it does not simply sit where it started.
        let turned = (0..24).any(|seed| {
            let park = run(
                Park::new("Weather", 32, 32, seed).unwrap(),
                Weather::SPELL * 6,
            );
            park.weather() != Weather::Sunny
        });
        assert!(turned, "the weather never changed for any seed");
    }

    #[test]
    fn fewer_people_turn_up_in_the_rain() {
        let mut fine = Park::new("Fine", 32, 32, 5).unwrap();
        let mut wet = Park::new("Wet", 32, 32, 5).unwrap();

        for _ in 0..Weather::SPELL {
            fine.set_weather(Weather::Sunny);
            fine.tick_once();

            wet.set_weather(Weather::Storm);
            wet.tick_once();
        }

        assert!(
            fine.guests().len() > wet.guests().len(),
            "{} turned up in the sun against {} in a storm",
            fine.guests().len(),
            wet.guests().len()
        );
    }

    #[test]
    fn the_crowd_gets_thirstier_in_the_sun_than_in_the_rain() {
        let thirst_of = |weather: Weather| {
            let mut park = Park::new("Thirsty", 32, 32, 5).unwrap();
            for tile in park.terrain().positions().collect::<Vec<_>>() {
                park.demolish(tile);
            }

            for _ in 0..6_000 {
                park.set_weather(weather);
                park.tick_once();
            }

            let total: f32 = park
                .guests()
                .iter()
                .map(|guest| guest.needs().thirst())
                .sum();
            #[allow(clippy::cast_precision_loss)]
            let average = total / park.guests().len().max(1) as f32;
            average
        };

        assert!(
            thirst_of(Weather::Sunny) > thirst_of(Weather::Rain),
            "a sunny afternoon was no thirstier than a wet one"
        );
    }

    #[test]
    fn standing_in_the_rain_puts_people_off_and_they_say_so() {
        let mut park = Park::new("Wet", 32, 32, 5).unwrap();
        for _ in 0..4_000 {
            park.set_weather(Weather::Storm);
            park.tick_once();
        }

        assert!(
            park.complaints()
                .iter()
                .any(|(thought, _)| *thought == Thought::Rain),
            "a storm and nobody mentioned it: {:?}",
            park.what_they_say()
        );
    }

    #[test]
    fn the_weather_survives_a_save() {
        let mut park = Park::new("Weather", 32, 32, 1).unwrap();
        park.set_weather(Weather::Rain);
        let park = run(park, 500);

        let json = serde_json::to_string(&park).unwrap();
        let loaded: Park = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.weather(), park.weather());
    }
}
