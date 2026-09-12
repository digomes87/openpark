//! Drawing the rides: the track, and the trains on it.
//!
//! A piece of track is drawn as two lines through the tile it sits on — one in
//! from the edge it is entered at, one out to the edge it leaves by — which
//! makes a curve read as a corner and a slope as a climb without needing
//! anything the [`Renderer`] does not already have. Trains follow the same path
//! at their own place along it.

use isogrid::camera::Camera;
use isogrid::iso::{GridPoint, ScreenPoint};
use isogrid::render::{Color, Renderer};

use crate::park::{Park, Ride, RideState, Segment, TrackPiece};

/// How thick the rails are drawn, as a fraction of a tile's width.
const RAIL: f32 = 0.1;

/// How thick a train is drawn.
const TRAIN: f32 = 0.3;

/// How tall the station platform stands.
const PLATFORM: f32 = 0.06;

/// The rails, by what the ride is doing.
const RUNNING: Color = Color::hex(0xD8_D2_C4);
const SHUT: Color = Color::hex(0x9A_92_84);
const UNBUILT: Color = Color::hex(0x6E_6A_62);
const FAULT: Color = Color::hex(0xE8_6A_5A);

/// The colour of a train, and of the platform beside a station.
const CARRIAGE: Color = Color::hex(0xC4_3B_3B);
const PLATFORM_COLOUR: Color = Color::hex(0xB2_A8_90);

/// Draws every ride in the park, back to front.
pub fn draw_rides(canvas: &mut dyn Renderer, park: &Park, camera: &Camera) {
    let scale = camera.tiles().width() * camera.zoom();

    // Sorted the way the land is: whatever is further down the screen last, so
    // a train in front of a hill stays in front of it.
    let mut order: Vec<(Segment, &Ride)> = park
        .rides()
        .iter()
        .flat_map(|ride| {
            ride.track()
                .segments()
                .into_iter()
                .map(move |laid| (laid, ride))
        })
        .collect();
    order.sort_by_key(|(laid, _)| (laid.tile.x + laid.tile.y, laid.tile.x, laid.entry));

    for (laid, ride) in order {
        draw_piece(canvas, camera, scale, laid, ride);
    }

    for ride in park.rides() {
        draw_trains(canvas, camera, scale, ride);
    }
}

/// One piece of track, and the platform if it is a station.
fn draw_piece(canvas: &mut dyn Renderer, camera: &Camera, scale: f32, laid: Segment, ride: &Ride) {
    let colour = match ride.state() {
        RideState::Open => RUNNING,
        RideState::Closed => SHUT,
        RideState::Building => UNBUILT,
        RideState::Broken => FAULT,
    };

    if laid.piece == TrackPiece::Station {
        let platform = camera.world_to_screen(along(laid, 0.5));
        canvas.line(
            ScreenPoint::new(platform.x - scale * 0.3, platform.y),
            ScreenPoint::new(platform.x + scale * 0.3, platform.y),
            PLATFORM * scale,
            PLATFORM_COLOUR,
        );
    }

    let (from, middle, to) = (along(laid, 0.0), along(laid, 0.5), along(laid, 1.0));
    for (a, b) in [(from, middle), (middle, to)] {
        canvas.line(
            camera.world_to_screen(a),
            camera.world_to_screen(b),
            RAIL * scale,
            colour,
        );
    }
}

/// Every train on one ride, wherever it has got to.
fn draw_trains(canvas: &mut dyn Renderer, camera: &Camera, scale: f32, ride: &Ride) {
    let laid = ride.track().segments();

    for train in ride.trains() {
        let Some(segment) = laid.get(train.at()) else {
            continue;
        };

        let at = camera.world_to_screen(along(*segment, train.progress()));
        let width = TRAIN * scale / 2.0;
        canvas.line(
            ScreenPoint::new(at.x - width, at.y),
            ScreenPoint::new(at.x + width, at.y),
            TRAIN * scale,
            CARRIAGE,
        );
    }
}

/// Where along a piece of track `t` is, from the edge it is entered at, through
/// the middle of the tile, to the edge it leaves by.
///
/// Height is interpolated with it, so a slope is a ramp rather than a step and a
/// train on one is drawn on the rails.
fn along(laid: Segment, t: f32) -> GridPoint {
    let centre = laid.tile.centre();
    let t = t.clamp(0.0, 1.0);

    let leaving = laid.piece.steer(laid.heading);
    let (in_x, in_y) = laid.heading.about().delta();
    let (out_x, out_y) = leaving.delta();

    #[allow(clippy::cast_precision_loss)]
    let entry = (centre.x + in_x as f32 / 2.0, centre.y + in_y as f32 / 2.0);
    #[allow(clippy::cast_precision_loss)]
    let exit = (centre.x + out_x as f32 / 2.0, centre.y + out_y as f32 / 2.0);

    let (from, to, across) = if t < 0.5 {
        (entry, (centre.x, centre.y), t * 2.0)
    } else {
        ((centre.x, centre.y), exit, (t - 0.5) * 2.0)
    };

    let climb = f32::from(laid.exit - laid.entry);
    GridPoint::new(
        from.0 + (to.0 - from.0) * across,
        from.1 + (to.1 - from.1) * across,
        f32::from(laid.entry) + climb * t,
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;
    use isogrid::camera::Viewport;
    use isogrid::iso::{TilePos, TileSize};
    use isogrid::render::{Command, Recorder};

    use crate::park::{Heading, Terrain};

    /// A park with one coaster on it, tested and open.
    fn fixture() -> (Park, Camera) {
        let mut park = Park::new("Rides", 32, 32, 5).expect("a valid park");
        park.adjust_cash(50_000);

        // Flat, dry ground to build on, so the layout is about the track
        // rather than about the lake and the hills underneath it.
        let corner = TilePos::new(10, 10);
        for dy in 0..6 {
            for dx in 0..6 {
                let tile = corner.offset(dx, dy);
                park.lay(tile, Terrain::Grass)
                    .expect("the ground should take grass");
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

        let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(1600.0, 1200.0).unwrap());
        camera.look_at(TilePos::new(12, 12).centre());
        (park, camera)
    }

    fn lines(canvas: &Recorder) -> usize {
        canvas
            .commands()
            .iter()
            .filter(|command| matches!(command, Command::Line(..)))
            .count()
    }

    #[test]
    fn a_park_with_no_rides_draws_nothing() {
        let park = Park::new("Empty", 16, 16, 1).unwrap();
        let camera = Camera::new(TileSize::CLASSIC, Viewport::new(800.0, 600.0).unwrap());

        let mut canvas = Recorder::new();
        draw_rides(&mut canvas, &park, &camera);
        assert!(canvas.commands().is_empty());
    }

    #[test]
    fn every_piece_of_track_is_drawn_in_two_halves() {
        let (park, camera) = fixture();
        let ride = &park.rides()[0];

        let mut canvas = Recorder::new();
        draw_rides(&mut canvas, &park, &camera);

        // Two rails a piece, one platform for the station, one train.
        let expected = ride.track().len() * 2 + ride.track().stations().len() + ride.trains().len();
        assert_eq!(lines(&canvas), expected);
    }

    #[test]
    fn a_piece_runs_from_the_edge_it_is_entered_at_to_the_one_it_leaves_by() {
        let straight = Segment {
            piece: TrackPiece::Straight,
            tile: TilePos::new(4, 4),
            heading: Heading::East,
            entry: 0,
            exit: 0,
        };

        let centre = straight.tile.centre();
        assert_eq!(along(straight, 0.0).x, centre.x - 0.5, "in from the west");
        assert_eq!(along(straight, 0.5).x, centre.x, "through the middle");
        assert_eq!(along(straight, 1.0).x, centre.x + 0.5, "out to the east");
    }

    #[test]
    fn a_curve_leaves_by_a_different_edge_than_it_came_in() {
        let curve = Segment {
            piece: TrackPiece::CurveRight,
            tile: TilePos::new(4, 4),
            heading: Heading::East,
            entry: 0,
            exit: 0,
        };

        let centre = curve.tile.centre();
        assert_eq!(along(curve, 0.0).x, centre.x - 0.5, "in from the west");
        assert_eq!(along(curve, 1.0).y, centre.y + 0.5, "out to the south");
        assert_eq!(along(curve, 1.0).x, centre.x, "and not straight on");
    }

    #[test]
    fn a_slope_is_a_ramp_rather_than_a_step() {
        let slope = Segment {
            piece: TrackPiece::SlopeUp,
            tile: TilePos::new(4, 4),
            heading: Heading::East,
            entry: 2,
            exit: 3,
        };

        assert_eq!(along(slope, 0.0).z, 2.0);
        assert_eq!(along(slope, 0.5).z, 2.5, "halfway up");
        assert_eq!(along(slope, 1.0).z, 3.0);
    }

    #[test]
    fn a_broken_ride_is_drawn_in_a_different_colour() {
        let (mut park, camera) = fixture();
        let id = park.rides()[0].id();

        let mut running = Recorder::new();
        draw_rides(&mut running, &park, &camera);

        park.close_ride(id).unwrap();
        let mut shut = Recorder::new();
        draw_rides(&mut shut, &park, &camera);

        assert_ne!(
            running.commands(),
            shut.commands(),
            "a shut ride looks exactly like a running one"
        );
    }
}
