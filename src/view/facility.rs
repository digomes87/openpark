//! Drawing what has been built on the land.
//!
//! A facility is painted onto its tile by the land pass, and then given a body
//! here: a few lines standing up out of the ground so a stall reads as a stall
//! from across the park rather than as a differently coloured square.

use isogrid::camera::Camera;
use isogrid::iso::{ScreenPoint, TilePos};
use isogrid::render::Renderer;

use crate::park::{Facility, Park};

/// How wide the body of a facility is, as a fraction of a tile's width.
const WIDTH: f32 = 0.42;

/// How thick its posts are drawn.
const POST: f32 = 0.09;

/// Draws everything built in the park, back to front.
pub fn draw_facilities(canvas: &mut dyn Renderer, park: &Park, camera: &Camera) {
    let scale = camera.tiles().width() * camera.zoom();

    for tile in park.facilities().draw_order() {
        let Some(facility) = park.facility_at(tile) else {
            continue;
        };

        let base = camera.world_to_screen(park.land().point(tile));
        if !is_on_screen(base, camera, scale) {
            continue;
        }

        draw_facility(canvas, base, scale, facility);
    }
}

/// Whether something standing at `base` is worth drawing at all.
fn is_on_screen(base: ScreenPoint, camera: &Camera, scale: f32) -> bool {
    let viewport = camera.viewport();
    base.x >= -scale
        && base.y >= -scale
        && base.x <= viewport.width() + scale
        && base.y <= viewport.height() + scale
}

/// One facility: two posts and a roof, sized by how tall it stands.
fn draw_facility(canvas: &mut dyn Renderer, base: ScreenPoint, scale: f32, facility: Facility) {
    let half = WIDTH * scale / 2.0;
    let height = facility.height() * scale;
    let thickness = POST * scale;

    let (left, right) = (base.x - half, base.x + half);
    let top = base.y - height;

    canvas.line(
        ScreenPoint::new(left, base.y),
        ScreenPoint::new(left, top),
        thickness,
        facility.colour(),
    );
    canvas.line(
        ScreenPoint::new(right, base.y),
        ScreenPoint::new(right, top),
        thickness,
        facility.colour(),
    );
    canvas.line(
        ScreenPoint::new(left, top),
        ScreenPoint::new(right, top),
        thickness,
        facility.trim_colour(),
    );
}

/// The label for the tile under the pointer, for the HUD to show.
///
/// Whoever is standing on the tile comes first, then whatever is built on it
/// with the price on its board and what it has taken, then the bare ground.
pub fn describe(park: &Park, tile: TilePos) -> String {
    if let Some(ride) = park.ride_at(tile) {
        let stats = ride.stats().map_or_else(
            || "not tested".to_owned(),
            |stats| {
                format!(
                    "excitement {:.0}%, intensity {:.0}%",
                    stats.excitement * 100.0,
                    stats.intensity * 100.0
                )
            },
        );

        return format!(
            "{} — {:?}, {} each, {stats}, {} ridden, {} queueing",
            ride.name(),
            ride.state(),
            ride.price(),
            ride.riders(),
            ride.queue().len()
        );
    }

    if let Some(member) = park.staff().iter().find(|member| member.tile() == tile) {
        return format!(
            "{} at {}, {} — {} a bill",
            member.kind().name(),
            tile.x,
            tile.y,
            member.wage()
        );
    }

    let steps = park.land().height_at(tile).unwrap_or_default();
    let height = if steps == 0 {
        String::new()
    } else {
        format!(" — {steps} steps up")
    };

    park.shop_at(tile).map_or_else(
        || {
            format!(
                "{} at {}, {}{height}",
                park.terrain()[tile].name(),
                tile.x,
                tile.y
            )
        },
        |shop| {
            format!(
                "{} at {}, {} — {} each, {} taken from {}",
                shop.kind().name(),
                tile.x,
                tile.y,
                shop.price(),
                shop.takings(),
                shop.customers()
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use isogrid::camera::Viewport;
    use isogrid::iso::TileSize;
    use isogrid::render::{Command, Recorder};

    fn fixture() -> (Park, Camera) {
        let park = Park::new("Built Up", 32, 32, 5).expect("a valid park");
        assert!(
            park.facilities().iter().any(|(_, built)| built.is_some()),
            "a new park comes with something on it"
        );

        let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(1600.0, 1200.0).unwrap());
        camera.look_at(TilePos::new(16, 16).centre());
        (park, camera)
    }

    fn lines(canvas: &Recorder) -> Vec<(ScreenPoint, ScreenPoint)> {
        canvas
            .commands()
            .iter()
            .filter_map(|command| match command {
                Command::Line(from, to, ..) => Some((*from, *to)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_park_with_nothing_built_draws_nothing() {
        let (mut park, camera) = fixture();
        for tile in park.terrain().positions().collect::<Vec<_>>() {
            park.demolish(tile);
        }

        let mut canvas = Recorder::new();
        draw_facilities(&mut canvas, &park, &camera);
        assert!(canvas.commands().is_empty());
    }

    #[test]
    fn every_facility_is_two_posts_and_a_roof() {
        let (park, camera) = fixture();
        let built = park
            .facilities()
            .iter()
            .filter(|(_, facility)| facility.is_some())
            .count();

        let mut canvas = Recorder::new();
        draw_facilities(&mut canvas, &park, &camera);
        assert_eq!(lines(&canvas).len(), built * 3);
    }

    #[test]
    fn a_facility_stands_up_out_of_its_tile() {
        let (park, camera) = fixture();
        let mut canvas = Recorder::new();
        draw_facilities(&mut canvas, &park, &camera);

        for post in lines(&canvas).chunks(3) {
            let (left, right, roof) = (post[0], post[1], post[2]);
            assert!(left.1.y < left.0.y, "the left post grows downwards");
            assert!(right.1.y < right.0.y, "the right post grows downwards");
            assert!(roof.0.x < roof.1.x, "the roof is not flat across the posts");
            assert!(
                (roof.0.y - left.1.y).abs() < f32::EPSILON,
                "the roof does not sit on the posts"
            );
        }
    }

    #[test]
    fn a_taller_facility_is_drawn_taller() {
        let mut park = Park::new("Two Things", 32, 32, 5).unwrap();
        for tile in park.terrain().positions().collect::<Vec<_>>() {
            park.demolish(tile);
        }

        let (stall, bench) = (TilePos::new(4, 4), TilePos::new(4, 6));
        park.build(stall, Facility::FoodStall).unwrap();
        park.build(bench, Facility::Bench).unwrap();

        let (_, camera) = fixture();
        let mut canvas = Recorder::new();
        draw_facilities(&mut canvas, &park, &camera);

        let heights: Vec<f32> = lines(&canvas)
            .chunks(3)
            .map(|post| post[0].0.y - post[0].1.y)
            .collect();
        assert_eq!(heights.len(), 2);
        let (tallest, shortest) = (
            heights.iter().copied().fold(f32::MIN, f32::max),
            heights.iter().copied().fold(f32::MAX, f32::min),
        );
        assert!(tallest > shortest, "a stall is no taller than a bench");
    }

    #[test]
    fn what_is_built_is_what_the_hud_names() {
        let (park, _) = fixture();
        let built = park
            .facilities()
            .iter()
            .find_map(|(tile, built)| built.map(|shop| (tile, shop)))
            .expect("something is built");

        assert!(describe(&park, built.0).starts_with(built.1.kind().name()));

        let empty = park
            .terrain()
            .positions()
            .find(|tile| park.facility_at(*tile).is_none())
            .expect("some ground is still bare");
        assert!(describe(&park, empty).contains(&format!("{}, {}", empty.x, empty.y)));
    }
}
