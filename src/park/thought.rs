//! What a guest is thinking.
//!
//! A park that is failing looks exactly like a park that is doing well until you
//! ask somebody. Every guest keeps the last thing it thought, and the park can
//! be asked what the crowd is saying — which is the difference between a number
//! going down and knowing why.
//!
//! Thoughts are set at the moment they happen, never derived after the fact: a
//! guest that gave up on a queue thinks so because it gave up on a queue, and it
//! goes on thinking it until something else happens to it.

use serde::{Deserialize, Serialize};

/// The last thing a guest thought.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Thought {
    /// Nothing in particular. What a guest thinks on the way in.
    #[default]
    LookingAround,
    /// It wants a drink and cannot find one.
    Thirsty,
    /// It wants something to eat and cannot find it.
    Hungry,
    /// It needs a toilet and there is not one.
    Bursting,
    /// It has been walking too long with nowhere to sit.
    Footsore,
    /// It came here to go on something and there is nothing to go on.
    Bored,
    /// It stood in a line that never moved.
    GaveUpQueueing(u32),
    /// It went on something and enjoyed it.
    LovedIt(u32),
    /// It went on something and would rather not have.
    Shaken(u32),
    /// It will not pay what something is asking.
    TooDear,
    /// It is standing in somebody else's rubbish.
    Litter,
    /// It has had enough and is leaving.
    GoingHome,
}

impl Thought {
    /// Whether this is a complaint rather than an observation.
    ///
    /// What the park's owner wants counted: nobody needs a list of the guests
    /// who are having a nice time.
    pub const fn is_a_complaint(self) -> bool {
        matches!(
            self,
            Self::Thirsty
                | Self::Hungry
                | Self::Bursting
                | Self::Footsore
                | Self::Bored
                | Self::GaveUpQueueing(_)
                | Self::Shaken(_)
                | Self::TooDear
                | Self::Litter
                | Self::GoingHome
        )
    }

    /// The ride this thought is about, if it is about one.
    pub const fn ride(self) -> Option<u32> {
        match self {
            Self::GaveUpQueueing(ride) | Self::LovedIt(ride) | Self::Shaken(ride) => Some(ride),
            _ => None,
        }
    }

    /// What the guest would say, with `ride` standing in for whatever it is
    /// about.
    ///
    /// The park fills the name in, because a thought does not know what rides
    /// are called.
    ///
    /// ```
    /// # use openpark::park::Thought;
    /// assert_eq!(Thought::Thirsty.said_about("—"), "I'm thirsty");
    /// assert_eq!(
    ///     Thought::LovedIt(0).said_about("The Wooden Hill"),
    ///     "The Wooden Hill was great"
    /// );
    /// ```
    pub fn said_about(self, ride: &str) -> String {
        match self {
            Self::LookingAround => "Just looking around".to_owned(),
            Self::Thirsty => "I'm thirsty".to_owned(),
            Self::Hungry => "I'm hungry".to_owned(),
            Self::Bursting => "I need the toilet".to_owned(),
            Self::Footsore => "My feet are killing me".to_owned(),
            Self::Bored => "There's nothing to go on".to_owned(),
            Self::GaveUpQueueing(_) => format!("The queue for {ride} is too long"),
            Self::LovedIt(_) => format!("{ride} was great"),
            Self::Shaken(_) => format!("{ride} shook me about"),
            Self::TooDear => "That's too expensive".to_owned(),
            Self::Litter => "This place is a tip".to_owned(),
            Self::GoingHome => "I'm going home".to_owned(),
        }
    }
}

impl crate::park::Park {
    /// What the crowd is saying, most-said first.
    ///
    /// Only the complaints, and only what somebody is thinking right now: a
    /// park's owner wants to know what is wrong with the place this minute, not
    /// a history of everybody who ever visited.
    ///
    /// ```
    /// # use openpark::park::Park;
    /// let park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// assert!(park.complaints().is_empty(), "an empty park has nobody to complain");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn complaints(&self) -> Vec<(Thought, usize)> {
        let mut said: Vec<(Thought, usize)> = Vec::new();

        for thought in self
            .guests()
            .iter()
            .map(crate::park::Guest::thought)
            .filter(|thought| thought.is_a_complaint())
        {
            match said.iter_mut().find(|(heard, _)| *heard == thought) {
                Some((_, count)) => *count += 1,
                None => said.push((thought, 1)),
            }
        }

        // Most-said first, and ties broken by what the thought is so that the
        // same park always reports the same order.
        said.sort_by(|(a, first), (b, second)| {
            second
                .cmp(first)
                .then_with(|| format!("{a:?}").cmp(&format!("{b:?}")))
        });
        said
    }

    /// What the crowd is saying, in words.
    ///
    /// The rides are named here rather than in [`Thought`], because a thought
    /// does not know what anything is called.
    pub fn what_they_say(&self) -> Vec<String> {
        self.complaints()
            .into_iter()
            .map(|(thought, count)| {
                let about = thought
                    .ride()
                    .and_then(|id| self.ride(id))
                    .map_or_else(|| "that ride".to_owned(), |ride| ride.name().to_owned());

                format!("{count} × {}", thought.said_about(&about))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every thought there is, so a new one cannot be added without the tests
    /// below seeing it.
    const EVERY: [Thought; 12] = [
        Thought::LookingAround,
        Thought::Thirsty,
        Thought::Hungry,
        Thought::Bursting,
        Thought::Footsore,
        Thought::Bored,
        Thought::GaveUpQueueing(0),
        Thought::LovedIt(0),
        Thought::Shaken(0),
        Thought::TooDear,
        Thought::Litter,
        Thought::GoingHome,
    ];

    #[test]
    fn a_guest_on_the_way_in_is_thinking_of_nothing_much() {
        assert_eq!(Thought::default(), Thought::LookingAround);
        assert!(!Thought::default().is_a_complaint());
    }

    #[test]
    fn everything_has_something_to_say() {
        for thought in EVERY {
            let said = thought.said_about("The Wooden Hill");
            assert!(!said.is_empty(), "{thought:?} says nothing");

            if thought.ride().is_some() {
                assert!(
                    said.contains("The Wooden Hill"),
                    "{thought:?} is about a ride and does not name it: {said}"
                );
            }
        }
    }

    #[test]
    fn enjoying_yourself_is_not_a_complaint() {
        assert!(!Thought::LovedIt(0).is_a_complaint());
        assert!(!Thought::LookingAround.is_a_complaint());

        for grumble in [
            Thought::Thirsty,
            Thought::Hungry,
            Thought::Bursting,
            Thought::Footsore,
            Thought::Bored,
            Thought::GaveUpQueueing(0),
            Thought::Shaken(0),
            Thought::TooDear,
            Thought::Litter,
            Thought::GoingHome,
        ] {
            assert!(grumble.is_a_complaint(), "{grumble:?}");
        }
    }

    #[test]
    fn only_the_thoughts_about_a_ride_carry_one() {
        assert_eq!(Thought::GaveUpQueueing(7).ride(), Some(7));
        assert_eq!(Thought::LovedIt(3).ride(), Some(3));
        assert_eq!(Thought::Shaken(1).ride(), Some(1));
        assert_eq!(Thought::Thirsty.ride(), None);
    }

    #[test]
    fn a_thought_survives_a_save() {
        for thought in EVERY {
            let json = serde_json::to_string(&thought).unwrap();
            assert_eq!(serde_json::from_str::<Thought>(&json).unwrap(), thought);
        }
    }
}
