//! A ride worth looking at, built without the mouse.
//!
//! The pictures in the README are taken by the game itself, and a coaster laid
//! by hand through a dozen clicks is not something a screenshot can reproduce.
//! So the layout the pictures use lives here, where it can be asked for with a
//! flag and asserted against in a test.

use anyhow::{Context, Result};
use isogrid::iso::TilePos;

use crate::park::{Heading, Park, Terrain, TrackPiece};

/// How far out from the crossroads the coaster is laid.
const OFFSET: i32 = 2;

/// How many tiles of ground are cleared for it.
const CLEARING: i32 = 6;

/// Lays a tested, open coaster beside the crossroads, and returns its id.
///
/// A chain lift three steps up, three drops back down, and brakes before the
/// station: the smallest layout that is recognisably a roller coaster rather
/// than a circle of track.
///
/// # Errors
///
/// Fails if the park is too small to hold it, or too poor to pay for it — the
/// caller is expected to have just built the park, so either is a bug rather
/// than a decision.
pub fn coaster(park: &mut Park) -> Result<u32> {
    #[allow(clippy::cast_possible_wrap)]
    let corner = TilePos::new(
        park.width() as i32 / 2 + OFFSET,
        park.height() as i32 / 2 + OFFSET,
    );

    // Flat, dry ground: the layout is meant to show off the track, not to
    // wrestle with whatever the generator put there.
    for dy in 0..CLEARING {
        for dx in 0..CLEARING {
            let tile = corner.offset(dx, dy);
            park.lay(tile, Terrain::Grass)
                .with_context(|| format!("{tile:?} will not take grass"))?;
            while park.land().height_at(tile).unwrap_or_default() > 0 {
                park.lower(tile)
                    .with_context(|| format!("{tile:?} will not come down"))?;
            }
        }
    }

    let id = park
        .start_a_ride("The Wooden Hill", corner, Heading::North)
        .context("there is nowhere to start a ride")?;

    for side in [
        [TrackPiece::Station, TrackPiece::LiftHill],
        [TrackPiece::LiftHill, TrackPiece::LiftHill],
        [TrackPiece::SlopeDown, TrackPiece::SlopeDown],
        [TrackPiece::SlopeDown, TrackPiece::Brakes],
    ] {
        park.lay_track(id, TrackPiece::CurveRight)
            .context("a corner would not lay")?;
        for piece in side {
            park.lay_track(id, piece)
                .with_context(|| format!("{} would not lay", piece.name()))?;
        }
    }

    park.test_ride(id).context("the demo coaster stalls")?;
    park.open_ride(id)
        .context("the demo coaster will not open")?;

    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::park::{Ride, RideState};

    #[test]
    fn the_demo_coaster_is_a_working_coaster() {
        let mut park = Park::new("Demo", 32, 32, 5).unwrap();
        park.adjust_cash(50_000);

        let id = coaster(&mut park).expect("the demo coaster should build");
        let ride = park.ride(id).expect("it should be there");

        assert_eq!(ride.state(), RideState::Open);
        assert!(ride.track().is_a_circuit());
        assert_eq!(ride.track().stations().len(), 1);

        let stats = ride.stats().expect("it was tested");
        assert!(stats.excitement > 0.0, "it is not a coaster if it is dull");
        assert_eq!(ride.track().longest_drop(), 2, "two drops in a row");
    }

    #[test]
    fn guests_ride_the_demo_coaster() {
        let mut park = Park::new("Demo", 32, 32, 5).unwrap();
        park.adjust_cash(50_000);
        let id = coaster(&mut park).unwrap();

        for _ in 0..20_000 {
            park.tick_once();
        }

        assert!(
            park.ride(id).is_some_and(|ride| ride.riders() > 0),
            "nobody rode the ride the screenshots are of"
        );
    }
}
