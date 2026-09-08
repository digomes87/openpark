//! The people a park is built for.
//!
//! A guest is a position and a route, nothing more yet. It knows how to walk
//! the route it was given; deciding where that route goes is the park's job,
//! which keeps the walking testable without a map.

use isogrid::iso::{GridPoint, TilePos};
use isogrid::render::Color;
use serde::{Deserialize, Serialize};

/// The shirts guests turn up in.
///
/// Placeholder art, like the terrain: flat colours picked to stay legible
/// against grass, path and water alike.
const SHIRTS: [Color; 6] = [
    Color::hex(0xD9_54_4D),
    Color::hex(0xE8_9C_2E),
    Color::hex(0xE8_D6_4A),
    Color::hex(0x4D_9D_D9),
    Color::hex(0x9B_5D_C4),
    Color::hex(0xE8_E4_DC),
];

/// One visitor.
///
/// Guests move between tile centres at a speed the park chooses, so a guest is
/// always somewhere on the segment between the tile it is standing on and the
/// next tile of its route.
///
/// ```
/// # use openpark::park::Guest;
/// # use isogrid::iso::TilePos;
/// let gate = TilePos::new(4, 0);
/// let mut guest = Guest::arriving(1, gate, 0);
/// assert!(guest.is_idle(), "a guest with nowhere to go stands still");
///
/// guest.follow(vec![gate, gate.offset(0, 1)])?;
/// guest.advance(0.5);
/// assert_eq!(guest.tile(), gate, "still half a tile from the next one");
/// assert!(guest.position().y > gate.centre().y, "but on its way");
/// # Ok::<(), anyhow::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Guest {
    id: u32,
    /// The tiles left to walk, starting with the one being stood on. Never
    /// empty: a guest is always somewhere.
    route: Vec<TilePos>,
    /// How far along `route` the guest is standing.
    step: usize,
    /// How far between `route[step]` and the tile after it, from 0 to 1.
    progress: f32,
    /// Which of [`SHIRTS`] this guest wears.
    shirt: u8,
}

impl Guest {
    /// A guest who has just walked through the gate at `at`.
    pub fn arriving(id: u32, at: TilePos, shirt: u8) -> Self {
        Self {
            id,
            route: vec![at],
            step: 0,
            progress: 0.0,
            shirt,
        }
    }

    /// Which guest this is. Unique within one park, and stable across a save.
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// The tile the guest is standing on, or walking away from.
    pub fn tile(&self) -> TilePos {
        self.route[self.step]
    }

    /// The tile the guest is walking towards, if it is walking anywhere.
    pub fn next_tile(&self) -> Option<TilePos> {
        self.route.get(self.step + 1).copied()
    }

    /// Whether the guest has run out of route and wants a new one.
    pub fn is_idle(&self) -> bool {
        self.next_tile().is_none()
    }

    /// Where the guest is, in grid space, between the two tiles of its step.
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

    /// The colour this guest draws as.
    pub fn shirt_colour(&self) -> Color {
        SHIRTS[self.shirt as usize % SHIRTS.len()]
    }

    /// Sends the guest off along a new route.
    ///
    /// The route must start where the guest is standing — a guest cannot
    /// teleport to the beginning of a path someone else walked.
    ///
    /// # Errors
    ///
    /// Fails if the route is empty or does not start at [`Guest::tile`].
    pub fn follow(&mut self, route: Vec<TilePos>) -> anyhow::Result<()> {
        anyhow::ensure!(!route.is_empty(), "a guest cannot follow an empty route");
        anyhow::ensure!(
            route[0] == self.tile(),
            "a route from {:?} does not start where guest {} is standing, at {:?}",
            route[0],
            self.id,
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
    /// leaves a guest standing rather than teleporting it somewhere strange.
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

    fn walking() -> Guest {
        let mut guest = Guest::arriving(1, TilePos::new(0, 0), 0);
        guest
            .follow(vec![
                TilePos::new(0, 0),
                TilePos::new(0, 1),
                TilePos::new(0, 2),
            ])
            .unwrap();
        guest
    }

    #[test]
    fn a_new_guest_stands_where_it_arrived_with_nowhere_to_go() {
        let guest = Guest::arriving(7, TilePos::new(4, 0), 2);
        assert_eq!(guest.id(), 7);
        assert_eq!(guest.tile(), TilePos::new(4, 0));
        assert!(guest.is_idle());
        assert_eq!(guest.position(), TilePos::new(4, 0).centre());
    }

    #[test]
    fn walking_moves_between_the_tiles_of_the_route() {
        let mut guest = walking();
        guest.advance(0.5);

        assert_eq!(guest.tile(), TilePos::new(0, 0));
        assert_eq!(guest.next_tile(), Some(TilePos::new(0, 1)));

        let half_way = guest.position();
        let (from, to) = (TilePos::new(0, 0).centre(), TilePos::new(0, 1).centre());
        assert!((half_way.y - f32::midpoint(from.y, to.y)).abs() < 1e-5);
        assert!((half_way.x - from.x).abs() < 1e-5);
    }

    #[test]
    fn a_long_stride_crosses_several_tiles_at_once() {
        let mut guest = walking();
        guest.advance(1.5);
        assert_eq!(guest.tile(), TilePos::new(0, 1));
        assert_eq!(guest.next_tile(), Some(TilePos::new(0, 2)));
    }

    #[test]
    fn a_guest_stops_at_the_end_of_its_route_however_hard_it_is_pushed() {
        let mut guest = walking();
        guest.advance(1_000.0);
        assert_eq!(guest.tile(), TilePos::new(0, 2));
        assert!(guest.is_idle());
        assert_eq!(guest.position(), TilePos::new(0, 2).centre());
    }

    #[test]
    fn a_nonsense_speed_leaves_the_guest_where_it_was() {
        let mut guest = walking();
        let before = guest.position();

        guest.advance(-1.0);
        guest.advance(f32::NAN);
        assert_eq!(guest.position(), before);
    }

    #[test]
    fn a_route_must_start_where_the_guest_is() {
        let mut guest = Guest::arriving(1, TilePos::new(0, 0), 0);
        assert!(guest.follow(vec![]).is_err());
        assert!(guest.follow(vec![TilePos::new(5, 5)]).is_err());
        assert!(guest.follow(vec![TilePos::new(0, 0)]).is_ok());
    }

    #[test]
    fn a_new_route_starts_from_the_beginning() {
        let mut guest = walking();
        guest.advance(1.5);

        let here = guest.tile();
        guest.follow(vec![here, here.offset(1, 0)]).unwrap();
        assert_eq!(guest.tile(), here);
        assert_eq!(guest.position(), here.centre());
    }

    #[test]
    fn guests_wear_one_of_the_shirts_however_high_the_number() {
        for shirt in 0..=u8::MAX {
            let guest = Guest::arriving(0, TilePos::ORIGIN, shirt);
            assert!(SHIRTS.contains(&guest.shirt_colour()));
        }
    }

    #[test]
    fn a_guest_survives_a_save() {
        let guest = walking();
        let json = serde_json::to_string(&guest).unwrap();
        assert_eq!(serde_json::from_str::<Guest>(&json).unwrap(), guest);
    }
}
