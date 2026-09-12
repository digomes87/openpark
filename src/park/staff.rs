//! The people the park pays.
//!
//! Staff walk the paths like guests do, but they are paid rather than paying,
//! and they change the park around them instead of wanting things from it. A
//! handyman puts the worn ground back; an entertainer keeps the crowd cheerful.
//! Mechanics arrive with the rides that need them.

use isogrid::iso::{GridPoint, TilePos};
use isogrid::render::Color;
use serde::{Deserialize, Serialize};

use crate::park::{Money, Walk};

/// A job somebody can be hired to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaffKind {
    /// Tidies the park: turns ground worn down to dirt back into grass.
    Handyman,
    /// Works the crowd: cheers up every guest walking near them.
    Entertainer,
    /// Fixes rides: puts a broken one back together, if they can get to it.
    Mechanic,
}

impl StaffKind {
    /// Everybody who can be hired, in the order a menu would list them.
    pub const ALL: [Self; 3] = [Self::Handyman, Self::Entertainer, Self::Mechanic];

    /// What it costs to take somebody on.
    ///
    /// A one-off, on top of the wages: the park pays for the uniform.
    pub const fn hire_cost(self) -> Money {
        match self {
            Self::Handyman => 150,
            Self::Entertainer => 250,
            Self::Mechanic => 300,
        }
    }

    /// What they are paid every wage bill.
    pub const fn wage(self) -> Money {
        match self {
            Self::Handyman => 40,
            Self::Entertainer => 60,
            Self::Mechanic => 80,
        }
    }

    /// How far their work reaches, in tiles.
    pub const fn reach(self) -> u32 {
        match self {
            Self::Handyman => 3,
            Self::Entertainer => 4,
            // Further than the rest: a mechanic walking the park should reach
            // the ride that broke down without being stood next to it.
            Self::Mechanic => 6,
        }
    }

    /// What to call them in the HUD.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Handyman => "Handyman",
            Self::Entertainer => "Entertainer",
            Self::Mechanic => "Mechanic",
        }
    }

    /// The uniform they draw in, so a crowd can be read at a glance.
    pub const fn colour(self) -> Color {
        match self {
            Self::Handyman => Color::hex(0x2F_6F_4F),
            Self::Entertainer => Color::hex(0xC0_3B_8F),
            Self::Mechanic => Color::hex(0x3B_5B_C0),
        }
    }
}

/// One member of staff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Staff {
    id: u32,
    kind: StaffKind,
    walk: Walk,
}

impl Staff {
    /// Somebody who has just been taken on, starting at `at`.
    pub fn hired(id: u32, kind: StaffKind, at: TilePos) -> Self {
        Self {
            id,
            kind,
            walk: Walk::standing_at(at),
        }
    }

    /// Which member of staff this is. Unique within one park.
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// What they were hired to do.
    pub const fn kind(&self) -> StaffKind {
        self.kind
    }

    /// What they are paid every wage bill.
    pub const fn wage(&self) -> Money {
        self.kind.wage()
    }

    /// The tile they are standing on, or walking away from.
    pub fn tile(&self) -> TilePos {
        self.walk.tile()
    }

    /// Where they are, between the two tiles of their step.
    pub fn position(&self) -> GridPoint {
        self.walk.position()
    }

    /// The tile they are walking towards, if they are walking anywhere.
    pub fn next_tile(&self) -> Option<TilePos> {
        self.walk.next_tile()
    }

    /// How far between their two tiles they are, from 0 to 1.
    pub const fn progress(&self) -> f32 {
        self.walk.progress()
    }

    /// Whether they have run out of route and want somewhere new to be.
    pub fn is_idle(&self) -> bool {
        self.walk.is_idle()
    }

    /// Whether `tile` is close enough for their work to reach it.
    ///
    /// ```
    /// # use openpark::park::{Staff, StaffKind};
    /// # use isogrid::iso::TilePos;
    /// let staff = Staff::hired(0, StaffKind::Handyman, TilePos::new(10, 10));
    /// assert!(staff.reaches(TilePos::new(11, 10)));
    /// assert!(!staff.reaches(TilePos::new(40, 40)));
    /// ```
    pub fn reaches(&self, tile: TilePos) -> bool {
        self.tile().manhattan_distance(tile) <= self.kind.reach()
    }

    /// Sends them off along a new route.
    ///
    /// # Errors
    ///
    /// Fails if the route is empty or does not start where they are standing.
    pub fn follow(&mut self, route: Vec<TilePos>) -> anyhow::Result<()> {
        self.walk.follow(route)
    }

    /// Walks `distance` tiles along their route.
    pub fn advance(&mut self, distance: f32) {
        self.walk.advance(distance);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nobody_works_for_nothing() {
        for kind in StaffKind::ALL {
            assert!(kind.wage() > 0, "{kind:?} works for free");
            assert!(kind.hire_cost() > 0, "{kind:?} costs nothing to take on");
            assert!(kind.reach() > 0, "{kind:?} cannot reach anything");
        }
    }

    #[test]
    fn everybody_is_told_apart_by_name_and_by_uniform() {
        let mut names: Vec<_> = StaffKind::ALL.iter().map(|kind| kind.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), StaffKind::ALL.len());

        let mut colours: Vec<_> = StaffKind::ALL.iter().map(|kind| kind.colour()).collect();
        colours.sort_by_key(|c| (c.r, c.g, c.b));
        colours.dedup();
        assert_eq!(colours.len(), StaffKind::ALL.len());
    }

    #[test]
    fn work_reaches_exactly_as_far_as_it_says() {
        let at = TilePos::new(10, 10);
        for kind in StaffKind::ALL {
            let staff = Staff::hired(0, kind, at);
            #[allow(clippy::cast_possible_wrap)]
            let edge = at.offset(kind.reach() as i32, 0);
            #[allow(clippy::cast_possible_wrap)]
            let past = at.offset(kind.reach() as i32 + 1, 0);

            assert!(staff.reaches(edge), "{kind:?} cannot reach its own edge");
            assert!(!staff.reaches(past), "{kind:?} reaches too far");
        }
    }

    #[test]
    fn somebody_just_hired_is_standing_where_they_were_put() {
        let staff = Staff::hired(7, StaffKind::Entertainer, TilePos::new(4, 0));
        assert_eq!(staff.id(), 7);
        assert_eq!(staff.tile(), TilePos::new(4, 0));
        assert!(staff.is_idle());
        assert_eq!(staff.wage(), StaffKind::Entertainer.wage());
    }

    #[test]
    fn a_member_of_staff_survives_a_save() {
        let staff = Staff::hired(3, StaffKind::Handyman, TilePos::new(1, 2));
        let json = serde_json::to_string(&staff).unwrap();
        assert_eq!(serde_json::from_str::<Staff>(&json).unwrap(), staff);
    }
}
