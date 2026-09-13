//! Parks to test against.
//!
//! The handful of parks the tests across this module all want: one that has been
//! open a while, one with nothing in it, one the bank has closed, and one with a
//! coaster on it. Shared rather than repeated, because a fixture copied into
//! four files drifts into four different fixtures.

use isogrid::iso::TilePos;

use crate::park::{Heading, Park, Terrain, TrackPiece};

/// How long a guest lasts in a park with nothing in it, near enough.
pub const A_WHOLE_VISIT: u64 = 20_000;

/// Runs a park for `ticks` ticks and hands it back.
pub fn opened_for(ticks: u64) -> Park {
    run(Park::new("Busy", 32, 32, 5).unwrap(), ticks)
}

/// Runs a park that has had everything torn down, so that guests have
/// nowhere to eat and nowhere to sit.
pub fn bare_park_opened_for(ticks: u64) -> Park {
    let mut park = Park::new("Bare", 32, 32, 5).unwrap();
    for tile in park.terrain().positions().collect::<Vec<_>>() {
        park.demolish(tile);
    }
    assert!(park.facilities().iter().all(|(_, built)| built.is_none()));
    run(park, ticks)
}

pub fn run(mut park: Park, ticks: u64) -> Park {
    for _ in 0..ticks {
        park.tick_once();
    }
    park
}

/// A park already past its debt limit, closed by the bank.
pub fn bankrupt_park() -> Park {
    let mut park = Park::new("Broke", 32, 32, 5).unwrap();
    park.adjust_cash(Park::DEBT_LIMIT * 2 - park.cash());
    let park = run(park, Park::TICKS_PER_WAGE_BILL);
    assert!(park.is_bankrupt());
    park
}

/// How much of the park is worn down to bare earth.
pub fn dirt(park: &Park) -> usize {
    park.terrain()
        .iter()
        .filter(|(_, ground)| **ground == Terrain::Dirt)
        .count()
}

/// A park with one tested, open coaster on flat dry ground beside the
/// crossroads, and the money to have built it.
pub fn park_with_a_coaster() -> (Park, u32) {
    let mut park = Park::new("Rides", 32, 32, 5).unwrap();
    park.adjust_cash(50_000);

    let corner = TilePos::new(18, 16);
    for dy in 0..6 {
        for dx in 0..6 {
            let tile = corner.offset(dx, dy);
            park.lay(tile, Terrain::Grass).expect("grass should lay");
            while park.land().height_at(tile).unwrap_or(0) > 0 {
                park.lower(tile).expect("the land should come down");
            }
        }
    }

    let id = park
        .start_a_ride("The Coaster", corner, Heading::North)
        .expect("a ride should start");
    for side in [
        [TrackPiece::Station, TrackPiece::LiftHill],
        [TrackPiece::LiftHill, TrackPiece::LiftHill],
        [TrackPiece::SlopeDown, TrackPiece::SlopeDown],
        [TrackPiece::SlopeDown, TrackPiece::Brakes],
    ] {
        park.lay_track(id, TrackPiece::CurveRight)
            .expect("a corner should lay");
        for piece in side {
            park.lay_track(id, piece).expect("a side should lay");
        }
    }

    park.test_ride(id).expect("the coaster should run");
    park.open_ride(id).expect("and open");
    (park, id)
}
