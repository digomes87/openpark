//! Getting from one tile to the next.
//!
//! Everybody in the park walks the same way: along a route of tiles, between
//! their centres, at whatever speed the park decides. Guests and staff differ
//! in why they are walking, never in how, so the how lives here on its own and
//! knows nothing about needs, wages or plans.

use isogrid::iso::{GridPoint, TilePos};
use serde::{Deserialize, Serialize};

/// A position on the grid, part way along a route.
///
/// ```
/// # use openpark::park::Walk;
/// # use isogrid::iso::TilePos;
/// let gate = TilePos::new(4, 0);
/// let mut walk = Walk::standing_at(gate);
/// assert!(walk.is_idle(), "somebody with nowhere to go stands still");
///
/// walk.follow(vec![gate, gate.offset(0, 1)])?;
/// walk.advance(0.5);
/// assert_eq!(walk.tile(), gate, "still half a tile from the next one");
/// assert!(walk.position().y > gate.centre().y, "but on its way");
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Walk {
    /// The tiles left to walk, starting with the one being stood on. Never
    /// empty: everybody is always somewhere.
    route: Vec<TilePos>,
    /// How far along `route` the walker is standing.
    step: usize,
    /// How far between `route[step]` and the tile after it, from 0 to 1.
    progress: f32,
}

impl Walk {
    /// Somebody standing on `tile` with nowhere to be.
    pub fn standing_at(tile: TilePos) -> Self {
        Self {
            route: vec![tile],
            step: 0,
            progress: 0.0,
        }
    }

    /// The tile being stood on, or walked away from.
    pub fn tile(&self) -> TilePos {
        self.route[self.step]
    }

    /// The tile being walked towards, if there is one.
    pub fn next_tile(&self) -> Option<TilePos> {
        self.route.get(self.step + 1).copied()
    }

    /// Whether the route has run out and a new one is wanted.
    pub fn is_idle(&self) -> bool {
        self.next_tile().is_none()
    }

    /// Where the walker is, in grid space, between the two tiles of its step.
    pub fn position(&self) -> GridPoint {
        let here = self.tile().centre();
        let Some(next) = self.next_tile() else {
            return here;
        };

        let there = next.centre();
        GridPoint::ground(
            here.x + (there.x - here.x) * self.progress,
            here.y + (there.y - here.y) * self.progress,
        )
    }

    /// Sets off along a new route.
    ///
    /// The route must start where the walker is standing — nobody can teleport
    /// to the beginning of a path somebody else walked.
    ///
    /// # Errors
    ///
    /// Fails if the route is empty or does not start at [`Walk::tile`].
    pub fn follow(&mut self, route: Vec<TilePos>) -> anyhow::Result<()> {
        anyhow::ensure!(!route.is_empty(), "nobody can follow an empty route");
        anyhow::ensure!(
            route[0] == self.tile(),
            "a route from {:?} does not start at {:?}, where the walker is",
            route[0],
            self.tile(),
        );

        self.route = route;
        self.step = 0;
        self.progress = 0.0;
        Ok(())
    }

    /// Walks `distance` tiles along the route, stopping at the end of it.
    ///
    /// Ignores a distance that is negative or not a number, so that a bad speed
    /// leaves somebody standing rather than teleporting them somewhere strange.
    pub fn advance(&mut self, distance: f32) {
        if distance.is_nan() || distance <= 0.0 {
            return;
        }

        let mut left = distance;
        while self.next_tile().is_some() {
            let to_the_next_tile = 1.0 - self.progress;
            if left < to_the_next_tile {
                self.progress += left;
                return;
            }

            left -= to_the_next_tile;
            self.step += 1;
            self.progress = 0.0;
        }

        // Out of route: stand on the last tile rather than past it.
        self.progress = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn somebody_standing_still_is_idle_and_on_their_tile() {
        let walk = Walk::standing_at(TilePos::new(3, 4));
        assert!(walk.is_idle());
        assert_eq!(walk.tile(), TilePos::new(3, 4));
        assert_eq!(walk.next_tile(), None);
        assert_eq!(walk.position(), TilePos::new(3, 4).centre());
    }

    #[test]
    fn a_route_has_to_start_where_the_walker_is() {
        let mut walk = Walk::standing_at(TilePos::ORIGIN);
        assert!(walk.follow(Vec::new()).is_err(), "an empty route");
        assert!(
            walk.follow(vec![TilePos::new(5, 5), TilePos::new(5, 6)])
                .is_err(),
            "a route that starts somewhere else"
        );
        assert!(walk
            .follow(vec![TilePos::ORIGIN, TilePos::new(0, 1)])
            .is_ok());
    }

    #[test]
    fn walking_the_whole_route_ends_on_the_last_tile() {
        let start = TilePos::ORIGIN;
        let mut walk = Walk::standing_at(start);
        walk.follow(vec![start, start.offset(0, 1), start.offset(0, 2)])
            .unwrap();

        walk.advance(10.0);
        assert_eq!(walk.tile(), start.offset(0, 2));
        assert!(walk.is_idle(), "and asks for somewhere new to go");
    }

    #[test]
    fn a_nonsense_speed_leaves_the_walker_where_it_was() {
        let start = TilePos::new(2, 2);
        let mut walk = Walk::standing_at(start);
        walk.follow(vec![start, start.offset(1, 0)]).unwrap();

        walk.advance(f32::NAN);
        walk.advance(-1.0);
        assert_eq!(walk.position(), start.centre());
    }
}
