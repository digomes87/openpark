//! Drawing what is planted.
//!
//! A stem and a crown: a trunk with a canopy over it reads as a tree, a short
//! post with a bright top reads as a lamp, and the same two lines with different
//! proportions cover everything in between. Sprites replace this later without
//! the rest of the view noticing, exactly as with the guests.

use isogrid::camera::Camera;
use isogrid::iso::ScreenPoint;
use isogrid::render::Renderer;

use crate::park::{Park, Scenery};

/// How thick the stem, trunk or post is drawn.
const STEM: f32 = 0.08;

/// How wide the crown is, as a fraction of a tile.
const CROWN: f32 = 0.34;

/// Draws everything planted in the park, back to front.
pub fn draw_scenery(canvas: &mut dyn Renderer, park: &Park, camera: &Camera) {
    let scale = camera.tiles().width() * camera.zoom();

    for tile in park.scenery().draw_order() {
        let Some(scenery) = park.scenery_at(tile) else {
            continue;
        };

        let base = camera.world_to_screen(park.land().point(tile));
        if !is_on_screen(base, camera, scale) {
            continue;
        }

        let height = scenery.height() * scale;
        let top = ScreenPoint::new(base.x, base.y - height);

        canvas.line(base, top, STEM * scale, scenery.stem_colour());
        canvas.line(
            ScreenPoint::new(base.x - CROWN * scale / 2.0, top.y),
            ScreenPoint::new(base.x + CROWN * scale / 2.0, top.y),
            CROWN * scale * crown_depth(scenery),
            scenery.colour(),
        );
    }
}

/// How deep the crown is drawn against its width.
///
/// A tree is round, a flowerbed is flat on the ground, and a lamp is a dot on a
/// stick.
fn crown_depth(scenery: Scenery) -> f32 {
    match scenery {
        Scenery::Tree => 0.9,
        Scenery::Flowerbed => 0.5,
        Scenery::Fountain => 0.7,
        Scenery::Lamp => 0.35,
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

#[cfg(test)]
mod tests {
    use super::*;
    use isogrid::camera::Viewport;
    use isogrid::iso::{TilePos, TileSize};
    use isogrid::render::{Command, Recorder};

    fn fixture() -> (Park, Camera) {
        let mut park = Park::new("Planted", 32, 32, 1).expect("a valid park");
        park.adjust_cash(10_000);

        let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(1600.0, 1200.0).unwrap());
        camera.look_at(TilePos::new(16, 16).centre());
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
    fn a_park_with_nothing_planted_draws_nothing() {
        let (park, camera) = fixture();
        let mut canvas = Recorder::new();
        draw_scenery(&mut canvas, &park, &camera);
        assert!(canvas.commands().is_empty());
    }

    #[test]
    fn everything_planted_is_a_stem_and_a_crown() {
        let (mut park, camera) = fixture();
        for (at, scenery) in Scenery::ALL.into_iter().enumerate() {
            #[allow(clippy::cast_possible_wrap)]
            let tile = TilePos::new(12 + at as i32, 13);
            park.plant(tile, scenery).expect("it should plant");
        }

        let mut canvas = Recorder::new();
        draw_scenery(&mut canvas, &park, &camera);
        assert_eq!(lines(&canvas), Scenery::ALL.len() * 2);
    }

    #[test]
    fn what_is_planted_on_a_hill_is_drawn_on_the_hill() {
        let (mut park, camera) = fixture();
        let tile = TilePos::new(13, 13);
        park.plant(tile, Scenery::Tree).expect("it should plant");

        let mut flat = Recorder::new();
        draw_scenery(&mut flat, &park, &camera);

        park.uproot(tile);
        park.raise(tile).expect("the land should move");
        park.plant(tile, Scenery::Tree)
            .expect("it should plant again");

        let mut raised = Recorder::new();
        draw_scenery(&mut raised, &park, &camera);
        assert_ne!(
            flat.commands(),
            raised.commands(),
            "a tree on a hill drew at ground level"
        );
    }

    #[test]
    fn nothing_off_screen_is_drawn() {
        let (mut park, mut camera) = fixture();
        park.plant(TilePos::new(13, 13), Scenery::Tree).unwrap();
        camera.look_at(TilePos::new(-400, -400).centre());

        let mut canvas = Recorder::new();
        draw_scenery(&mut canvas, &park, &camera);
        assert!(canvas.commands().is_empty());
    }
}
