//! What the park is worth, and what people think of it.
//!
//! Two numbers that answer different questions. **Value** is what is standing on
//! the land: build more and it goes up, and it does not care whether anybody is
//! enjoying any of it. **Rating** is what a visitor would say about the place —
//! how much there is to do, how it looks, how worn it is, and how the crowd that
//! is already inside is feeling.
//!
//! Rating is the one that matters, because it decides how many people turn up.
//! A park that builds and builds without looking after what it has ends up rich
//! in value and empty of guests.

use isogrid::iso::TilePos;
use serde::{Deserialize, Serialize};

use crate::park::{Money, Park, Scenery, Terrain};

/// What people think of a park, and what it is worth.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rating {
    /// What the crowd would give the place, from 0 to
    /// [`Rating::BEST`].
    pub rating: u32,
    /// What everything standing on the land cost to build.
    pub value: Money,
    /// How much there is to do, from 0 to 1.
    pub things_to_do: f32,
    /// How it looks, from 0 to 1.
    pub looks: f32,
    /// How the crowd inside is feeling, from 0 to 1. `None` when there is
    /// nobody to ask.
    pub mood: Option<f32>,
}

impl Rating {
    /// The best a park can be rated.
    ///
    /// The number is arbitrary; it is a thousand because a rating out of a
    /// thousand reads as a score and a rating out of one reads as a fraction.
    pub const BEST: u32 = 1_000;

    /// How much of the rating comes from having something to do.
    const RIDES_ARE_WORTH: f32 = 0.45;

    /// How much comes from the place being worth looking at.
    const LOOKS_ARE_WORTH: f32 = 0.25;

    /// How much comes from the mood of the people already inside.
    const MOOD_IS_WORTH: f32 = 0.30;

    /// How many open rides it takes before a park counts as having plenty.
    const PLENTY_OF_RIDES: f32 = 6.0;

    /// How much beauty a tile needs before it counts as fully looked after.
    const HANDSOME: f32 = 12.0;

    /// What a park with nothing in it is rated.
    ///
    /// Not zero: an empty field on a sunny day is worth something, and a rating
    /// of nothing would mean nobody ever came to see the first ride.
    const A_FIELD: f32 = 0.1;
}

impl Park {
    /// What the park is worth and what people think of it.
    ///
    /// ```
    /// # use openpark::park::{Park, Rating, Scenery};
    /// # use isogrid::iso::TilePos;
    /// let mut park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// let bare = park.rating().rating;
    ///
    /// for x in 4..8 {
    ///     park.plant(TilePos::new(x, 17), Scenery::Tree)?;
    /// }
    /// assert!(park.rating().rating > bare, "trees are worth planting");
    /// assert!(park.rating().rating <= Rating::BEST);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn rating(&self) -> Rating {
        let things_to_do = self.things_to_do();
        let looks = self.looks();
        let mood = self.average_happiness();

        // With nobody inside, the mood share is what the park itself promises
        // rather than an average of no opinions — otherwise an empty park would
        // score worse than the same park with one cheerful guest in it.
        let felt = mood.unwrap_or_else(|| f32::midpoint(things_to_do, looks));
        let score = Rating::A_FIELD
            + things_to_do * Rating::RIDES_ARE_WORTH
            + looks * Rating::LOOKS_ARE_WORTH
            + felt * Rating::MOOD_IS_WORTH;

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let rating = (score.clamp(0.0, 1.0) * Rating::BEST as f32) as u32;

        Rating {
            rating,
            value: self.value(),
            things_to_do,
            looks,
            mood,
        }
    }

    /// What everything standing on the land cost to build, from 0 upwards.
    ///
    /// What it cost rather than what it is worth now: a park's value is the sum
    /// of the decisions made on it, and a ride does not become a better ride for
    /// having been open a while.
    pub fn value(&self) -> Money {
        let built: Money = self
            .facilities()
            .iter()
            .filter_map(|(_, built)| built.as_ref())
            .map(|shop| shop.kind().build_cost())
            .sum();
        let planted: Money = self
            .scenery()
            .iter()
            .filter_map(|(_, planted)| planted.map(Scenery::cost))
            .sum();
        let rides: Money = self.rides().iter().map(|ride| ride.track().cost()).sum();

        built.saturating_add(planted).saturating_add(rides)
    }

    /// How much there is to do here, from 0 to 1.
    ///
    /// Open rides, weighted by how exciting they turned out to be: six dull
    /// rides are not the same afternoon as six good ones, and a ride nobody can
    /// get on is not an afternoon at all.
    fn things_to_do(&self) -> f32 {
        let worth: f32 = self
            .rides()
            .iter()
            .filter(|ride| ride.is_open())
            .map(|ride| {
                ride.stats()
                    .map_or(0.0, |stats| 0.4 + stats.excitement * 0.6)
            })
            .sum();

        (worth / Rating::PLENTY_OF_RIDES).clamp(0.0, 1.0)
    }

    /// How the park looks, from 0 to 1.
    ///
    /// Measured where it is looked at: beauty is counted around the paths people
    /// walk, and ground worn down to dirt counts against it. A forest planted in
    /// a corner nobody visits does nothing, which is the right answer.
    fn looks(&self) -> f32 {
        let mut looked_at = 0u32;
        let mut beauty = 0u32;
        let mut worn = 0u32;

        for (tile, ground) in self.terrain().iter() {
            match ground {
                Terrain::Path | Terrain::Queue => looked_at += 1,
                Terrain::Dirt => worn += 1,
                _ => continue,
            }

            if *ground == Terrain::Dirt {
                continue;
            }

            beauty += self.beauty_around(tile);
        }

        if looked_at == 0 {
            return 0.0;
        }

        #[allow(clippy::cast_precision_loss)]
        let pretty = beauty as f32 / (looked_at as f32 * Rating::HANDSOME);
        #[allow(clippy::cast_precision_loss)]
        let shabby = worn as f32 / (looked_at as f32 * 2.0);

        (pretty - shabby).clamp(0.0, 1.0)
    }

    /// How much scenery can be seen from `tile`.
    fn beauty_around(&self, tile: TilePos) -> u32 {
        self.scenery()
            .iter()
            .filter_map(|(at, planted)| planted.map(|scenery| (at, scenery)))
            .filter(|(at, scenery)| tile.manhattan_distance(*at) <= scenery.reach())
            .map(|(_, scenery)| scenery.beauty())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::park::fixtures::park_with_a_coaster;
    use crate::park::Facility;

    /// Trees along the crossroads, where the crowd actually walks.
    fn plant_a_few(park: &mut Park, scenery: Scenery, how_many: i32) {
        park.adjust_cash(10_000);
        #[allow(clippy::cast_possible_wrap)]
        let middle = park.height() as i32 / 2;

        let mut planted = 0;
        for x in 0..park.width() {
            #[allow(clippy::cast_possible_wrap)]
            let tile = TilePos::new(x as i32, middle + 1);
            if planted >= how_many {
                break;
            }
            if park.plant(tile, scenery).is_ok() {
                planted += 1;
            }
        }

        assert_eq!(planted, how_many, "there was nowhere to plant");
    }

    #[test]
    fn a_new_park_is_rated_but_not_highly() {
        let park = Park::new("Forest Frontiers", 32, 32, 1).unwrap();
        let rating = park.rating();

        assert!(rating.rating > 0, "an empty field is worth something");
        assert!(
            rating.rating < Rating::BEST / 2,
            "a park with no rides scored {}",
            rating.rating
        );
        assert_eq!(rating.things_to_do, 0.0, "there is nothing to do");
        assert_eq!(rating.mood, None, "nobody is there to ask");
    }

    #[test]
    fn planting_where_people_walk_raises_the_rating() {
        let mut park = Park::new("Forest Frontiers", 32, 32, 1).unwrap();
        let bare = park.rating();

        plant_a_few(&mut park, Scenery::Tree, 6);
        let planted = park.rating();

        assert!(
            planted.looks > bare.looks,
            "six trees along the path changed nothing"
        );
        assert!(planted.rating > bare.rating);
        assert!(planted.value > bare.value, "they cost money to put up");
    }

    #[test]
    fn a_fountain_is_worth_more_than_a_lamp() {
        let mut with_a_fountain = Park::new("Forest Frontiers", 32, 32, 1).unwrap();
        let mut with_a_lamp = Park::new("Forest Frontiers", 32, 32, 1).unwrap();

        plant_a_few(&mut with_a_fountain, Scenery::Fountain, 1);
        plant_a_few(&mut with_a_lamp, Scenery::Lamp, 1);

        assert!(with_a_fountain.rating().looks > with_a_lamp.rating().looks);
    }

    #[test]
    fn scenery_nobody_walks_past_does_nothing_for_the_rating() {
        let mut park = Park::new("Forest Frontiers", 32, 32, 1).unwrap();
        park.adjust_cash(10_000);
        let bare = park.rating();

        // The far corner, well away from the crossroads.
        for x in 0..3 {
            let _ = park.plant(TilePos::new(x, 0), Scenery::Fountain);
        }

        let hidden = park.rating();
        assert_eq!(
            hidden.looks, bare.looks,
            "a fountain in a corner nobody visits raised the rating"
        );
        assert!(hidden.value > bare.value, "it still cost money");
    }

    #[test]
    fn a_park_with_a_ride_has_more_to_do_than_one_without() {
        let (with_a_ride, _) = park_with_a_coaster();
        let without = Park::new("Rides", 32, 32, 5).unwrap();

        assert!(with_a_ride.rating().things_to_do > without.rating().things_to_do);
        assert!(with_a_ride.rating().rating > without.rating().rating);
    }

    #[test]
    fn a_shut_ride_is_nothing_to_do() {
        let (mut park, id) = park_with_a_coaster();
        let open = park.rating();

        park.close_ride(id).unwrap();
        let shut = park.rating();

        assert!(
            shut.things_to_do < open.things_to_do,
            "a shut ride still counted"
        );
        assert_eq!(shut.value, open.value, "it is still standing there");
    }

    #[test]
    fn worn_out_ground_counts_against_the_looks() {
        let mut park = Park::new("Forest Frontiers", 32, 32, 1).unwrap();
        plant_a_few(&mut park, Scenery::Tree, 6);
        let tidy = park.rating();

        #[allow(clippy::cast_possible_wrap)]
        let middle = park.height() as i32 / 2;
        for x in 0..park.width() {
            #[allow(clippy::cast_possible_wrap)]
            park.set_terrain(TilePos::new(x as i32, middle - 1), Terrain::Dirt);
        }

        assert!(
            park.rating().looks < tidy.looks,
            "a park worn down to dirt looked just as good"
        );
    }

    #[test]
    fn value_is_what_was_built_rather_than_what_it_earns() {
        let mut park = Park::new("Forest Frontiers", 32, 32, 1).unwrap();
        let before = park.value();

        park.build(TilePos::new(2, 2), Facility::FoodStall).unwrap();
        assert_eq!(park.value(), before + Facility::FoodStall.build_cost());

        // A busy day changes the takings, not the value.
        let park = crate::park::fixtures::run(park, 5_000);
        assert_eq!(park.value(), before + Facility::FoodStall.build_cost());
    }

    #[test]
    fn a_rating_never_leaves_its_range() {
        let (mut park, _) = park_with_a_coaster();
        park.adjust_cash(1_000_000);
        plant_a_few(&mut park, Scenery::Fountain, 8);

        let rating = park.rating();
        assert!(rating.rating <= Rating::BEST);
        assert!((0.0..=1.0).contains(&rating.looks));
        assert!((0.0..=1.0).contains(&rating.things_to_do));
    }
}
