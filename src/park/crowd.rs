//! How the crowd finds its way about.
//!
//! The park as the pathfinder sees it, and the handful of searches a guest runs
//! on it: the nearest stall it will pay for, the nearest ride it fancies, and
//! somewhere to wander when it wants neither. Kept apart from the park itself
//! because none of it changes anything — every function here looks at a park and
//! gives back a route.

use core::num::NonZeroU32;

use isogrid::grid::{Grid, TileBounds};
use isogrid::iso::TilePos;
use isogrid::path::{PathFinder, Traversable};
use isogrid::rng::Rng;

use crate::park::{Facility, Guest, Land, Park, Ride, Shop, Terrain};

/// The park as the pathfinder sees it: ground that can be crossed, minus
/// whatever has been built on it.
///
/// A facility blocks its own tile, which is what makes guests queue beside a
/// stall rather than walk through it.
pub struct ParkMap<'a> {
    /// The ground, and the shape of it.
    pub land: &'a Land,
    /// What is built on it, which blocks the tile it stands on.
    pub facilities: &'a Grid<Option<Shop>>,
}

impl Traversable for ParkMap<'_> {
    fn bounds(&self) -> TileBounds {
        self.land.terrain().bounds()
    }

    fn step_cost(&self, from: TilePos, to: TilePos) -> Option<NonZeroU32> {
        if self.facilities.get(to)?.is_some() {
            return None;
        }

        // The shape of the land decides the rest: water and cliffs are refused,
        // and a climb costs more than the same distance on the flat.
        self.land.step_cost(from, to)
    }
}

/// Whether a tile of grass is somewhere wear would actually start.
///
/// Only grass next to a path or to ground already worn down: wear spreads from
/// the edges of where people are already walking, which is what turns it into
/// trails across the lawn rather than a rash of bare patches all over it.
pub fn wears_from_here(land: &Land, tile: TilePos) -> bool {
    tile.neighbours()
        .iter()
        .any(|beside| matches!(land.ground(*beside), Some(Terrain::Path | Terrain::Dirt)))
}

/// What a guest would go out of its way for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wanted {
    /// A go on something.
    Ride,
    /// A stall or a bench.
    Something(Facility),
}

/// Finds the nearest ride a guest at `from` can get to, get on, and afford.
///
/// Returns the ride, the station tile to board from, and the route to the tile
/// beside it. Only the [`Park::FACILITY_ATTEMPTS`] nearest are tried, for the
/// same reason as stalls: past that the walk is long enough that the guest may
/// as well wander and look again.
pub fn nearest_ride(
    map: &ParkMap<'_>,
    finder: &mut PathFinder,
    from: TilePos,
    rides: &[Ride],
    guest: &Guest,
) -> Option<(u32, TilePos, Vec<TilePos>)> {
    let mut candidates: Vec<(u32, u32, TilePos)> = rides
        .iter()
        .filter(|ride| ride.is_open() && guest.will_ride(ride))
        .flat_map(|ride| {
            ride.track()
                .stations()
                .into_iter()
                .map(move |station| (from.manhattan_distance(station), ride.id(), station))
        })
        .collect();
    candidates.sort_unstable();

    for (_, ride, station) in candidates.into_iter().take(Park::FACILITY_ATTEMPTS) {
        // Guests board from the tile beside the station, never off the track.
        for beside in station.neighbours() {
            if let Some(route) = finder.find(map, from, beside) {
                return Some((ride, station, route.tiles().to_vec()));
            }
        }
    }

    None
}

/// Finds the nearest facility of a kind that a guest at `from` can actually
/// walk up to and will pay for, and the route to the tile it would stand on.
///
/// Only the [`Park::FACILITY_ATTEMPTS`] nearest are tried: past that the walk
/// is long enough that the guest may as well wander and ask again later.
/// Anything charging more than the guest thinks it is worth is not a candidate
/// at all — an overpriced stall is invisible rather than disappointing.
pub fn nearest_facility(
    map: &ParkMap<'_>,
    finder: &mut PathFinder,
    from: TilePos,
    wanted: Facility,
    guest: &Guest,
) -> Option<(TilePos, Vec<TilePos>)> {
    let mut candidates: Vec<(u32, TilePos)> = map
        .facilities
        .iter()
        .filter_map(|(tile, built)| {
            let shop = (*built)?;
            (shop.kind() == wanted && guest.will_pay(&shop))
                .then_some((from.manhattan_distance(tile), tile))
        })
        .collect();
    candidates.sort_unstable();

    for (_, facility) in candidates.into_iter().take(Park::FACILITY_ATTEMPTS) {
        // The counter is beside the stall, never on it.
        for beside in facility.neighbours() {
            if let Some(route) = finder.find(map, from, beside) {
                return Some((facility, route.tiles().to_vec()));
            }
        }
    }

    None
}

/// Picks somewhere for a guest at `from` to go, and works out how to get there.
///
/// Returns `None` if nowhere reachable turned up in [`Park::WANDER_ATTEMPTS`]
/// tries, which leaves the guest standing for a tick and trying again on the
/// next one — cheaper than searching a whole park for the one open tile.
pub fn wander(
    map: &ParkMap<'_>,
    rng: &mut Rng,
    finder: &mut PathFinder,
    from: TilePos,
) -> Option<Vec<TilePos>> {
    for attempt in 0..Park::WANDER_ATTEMPTS {
        #[allow(clippy::cast_possible_wrap)]
        let goal = TilePos::new(
            rng.below(map.land.width())? as i32,
            rng.below(map.land.height())? as i32,
        );

        if goal == from {
            continue;
        }

        let ground = map.land.ground(goal)?;
        let wanted = if attempt < Park::PATH_ATTEMPTS {
            ground == Terrain::Path
        } else {
            ground.is_walkable()
        };

        if wanted {
            if let Some(route) = finder.find(map, from, goal) {
                return Some(route.tiles().to_vec());
            }
        }
    }

    None
}
