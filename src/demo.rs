//! A ride worth looking at, built without the mouse.
//!
//! The pictures in the README are taken by the game itself, and a coaster laid
//! by hand through a dozen clicks is not something a screenshot can reproduce.
//! So the layout the pictures use lives here, where it can be asked for with a
//! flag and asserted against in a test.

use anyhow::{Context, Result};
use isogrid::iso::TilePos;

use crate::park::{Heading, Park, Scenery, Terrain, TrackPiece};

/// How far out from the crossroads the coaster is laid.
const OFFSET: i32 = 2;

/// How many tiles of ground are cleared for it.
const CLEARING: i32 = 6;

/// How long a queue is laid for it.
const QUEUE_LENGTH: i32 = 5;

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

    queue_for(park, id).context("the demo coaster has nowhere to queue")?;
    plant_around(park);

    park.test_ride(id).context("the demo coaster stalls")?;
    park.open_ride(id)
        .context("the demo coaster will not open")?;

    Ok(id)
}

/// Plants a row of scenery along the crossroads, where the crowd walks.
///
/// Where it is looked at, in other words: the rating counts beauty around paths
/// rather than beauty anywhere, so scenery in the corner of the map would be a
/// picture of the feature not working.
///
/// Whatever lands in the lake or under the coaster is simply skipped: this is
/// decoration, and a demo park that refused to build because one flowerbed had
/// nowhere to go would be no demo at all.
fn plant_around(park: &mut Park) {
    #[allow(clippy::cast_possible_wrap)]
    let middle = park.height() as i32 / 2;

    for x in 0..park.width() {
        #[allow(clippy::cast_possible_wrap)]
        let x = x as i32;
        if x % 3 != 0 {
            continue;
        }

        // Either side of the path across the park, and never mind the ones that
        // land in the lake or under the coaster.
        for tile in [TilePos::new(x, middle - 1), TilePos::new(x, middle + 1)] {
            let planted = if x % 6 == 0 {
                Scenery::Tree
            } else {
                Scenery::Flowerbed
            };
            let _ = park.plant(tile, planted);
        }
    }
}

/// Lays a queue path leading away from a ride's station.
///
/// Away from the ride rather than into the middle of it: the obvious neighbour
/// of a station on a ring layout is the inside of the ring, which runs out of
/// room after a tile or two.
///
/// # Errors
///
/// Fails if the ride has no station, or if the ground beside it will not take a
/// queue.
fn queue_for(park: &mut Park, ride: u32) -> Result<()> {
    let station = *park
        .ride(ride)
        .context("there is no such ride")?
        .track()
        .stations()
        .first()
        .context("the ride has no station")?;

    let room = |park: &Park, (dx, dy): (i32, i32)| {
        let mut tile = station;
        let mut count = 0;
        while count < QUEUE_LENGTH {
            let next = tile.offset(dx, dy);
            if park.ride_at(next).is_some() || !park.land().contains(next) {
                break;
            }
            count += 1;
            tile = next;
        }
        count
    };

    let way = [(0, -1), (1, 0), (0, 1), (-1, 0)]
        .into_iter()
        .max_by_key(|way| room(park, *way))
        .context("a station with no sides")?;

    let mut tile = station;
    for _ in 0..room(park, way) {
        tile = tile.offset(way.0, way.1);
        park.lay(tile, Terrain::Queue)
            .with_context(|| format!("{tile:?} will not take a queue"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::park::RideState;

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

        let queue = park
            .terrain()
            .iter()
            .filter(|(_, ground)| **ground == Terrain::Queue)
            .count();
        assert!(
            queue >= 3,
            "the demo ride has nowhere to queue: {queue} tiles"
        );
    }

    #[test]
    fn guests_ride_the_demo_coaster() {
        let mut park = Park::new("Demo", 32, 32, 5).unwrap();
        park.adjust_cash(50_000);
        let id = coaster(&mut park).unwrap();

        let mut ever_queued = false;
        for _ in 0..20_000 {
            park.tick_once();
            ever_queued |= park.ride(id).is_some_and(|ride| !ride.queue().is_empty());
        }

        assert!(
            park.ride(id).is_some_and(|ride| ride.riders() > 0),
            "nobody rode the ride the screenshots are of"
        );
        // Checked across the whole day rather than at the end of it: a line
        // that is empty right now is a line the train has just cleared, which
        // is the queue working rather than the queue missing.
        assert!(ever_queued, "nobody ever stood in the queue that was laid");
    }
}
