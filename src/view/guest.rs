//! Drawing the crowd.
//!
//! Guests are placeholder art like everything else: a shadow, a body and a
//! head, built out of three lines so they scale with the zoom and cost nothing
//! to draw. Sprites replace this later without the rest of the view noticing.

use isogrid::camera::Camera;
use isogrid::iso::ScreenPoint;
use isogrid::render::{Color, Renderer};

use crate::park::{Guest, Park};

/// What a guest casts on the ground.
const SHADOW: Color = Color::rgba(0, 0, 0, 80);

/// The colour of a head.
const SKIN: Color = Color::hex(0xE8_C3_9E);

/// Every measurement below is a fraction of a tile's width on screen, so a
/// guest stays the same size relative to the land at any zoom.
const BODY_HEIGHT: f32 = 0.28;
const HEAD_HEIGHT: f32 = 0.13;
const BODY_WIDTH: f32 = 0.13;
const SHADOW_WIDTH: f32 = 0.20;

/// Draws everyone in the park, nearest last so that the crowd overlaps the way
/// the land does.
pub fn draw_guests(canvas: &mut dyn Renderer, park: &Park, camera: &Camera) {
    let scale = camera.tiles().width() * camera.zoom();

    let mut crowd: Vec<(ScreenPoint, &Guest)> = park
        .guests()
        .iter()
        .map(|guest| (camera.world_to_screen(guest.position()), guest))
        .filter(|(feet, _)| is_on_screen(*feet, camera, scale))
        .collect();

    // Sorted by how far down the screen they stand, which in an isometric view
    // is the same as how close to the camera they are.
    crowd.sort_by(|(a, _), (b, _)| a.y.total_cmp(&b.y));

    for (feet, guest) in crowd {
        draw_guest(canvas, feet, scale, guest.shirt_colour());
    }
}

/// Whether a guest standing at `feet` is worth drawing at all.
fn is_on_screen(feet: ScreenPoint, camera: &Camera, scale: f32) -> bool {
    let viewport = camera.viewport();
    feet.x >= -scale
        && feet.y >= -scale
        && feet.x <= viewport.width() + scale
        && feet.y <= viewport.height() + scale
}

/// One guest: a shadow under the feet, a body, and a head on top of it.
fn draw_guest(canvas: &mut dyn Renderer, feet: ScreenPoint, scale: f32, shirt: Color) {
    let half_shadow = SHADOW_WIDTH * scale / 2.0;
    canvas.line(
        ScreenPoint::new(feet.x - half_shadow, feet.y),
        ScreenPoint::new(feet.x + half_shadow, feet.y),
        BODY_WIDTH * scale * 0.6,
        SHADOW,
    );

    let shoulders = ScreenPoint::new(feet.x, feet.y - BODY_HEIGHT * scale);
    canvas.line(feet, shoulders, BODY_WIDTH * scale, shirt);

    let crown = ScreenPoint::new(feet.x, shoulders.y - HEAD_HEIGHT * scale);
    canvas.line(shoulders, crown, BODY_WIDTH * scale * 0.8, SKIN);
}

#[cfg(test)]
mod tests {
    use super::*;
    use isogrid::camera::Viewport;
    use isogrid::iso::{TilePos, TileSize};
    use isogrid::render::{Command, Recorder};

    /// A park that has been open long enough to have a crowd in it.
    fn busy() -> (Park, Camera) {
        let mut park = Park::new("Busy Park", 32, 32, 5).expect("a valid park");
        for _ in 0..600 {
            park.tick_once();
        }
        assert!(!park.guests().is_empty(), "nobody turned up to be drawn");

        let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(800.0, 600.0).unwrap());
        camera.look_at(TilePos::new(16, 16).centre());
        (park, camera)
    }

    fn lines(canvas: &Recorder) -> Vec<(ScreenPoint, ScreenPoint, Color)> {
        canvas
            .commands()
            .iter()
            .filter_map(|command| match command {
                Command::Line(from, to, _, colour) => Some((*from, *to, *colour)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn an_empty_park_draws_nobody() {
        let park = Park::new("Empty", 16, 16, 1).unwrap();
        let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(800.0, 600.0).unwrap());
        camera.look_at(TilePos::new(8, 8).centre());

        let mut canvas = Recorder::new();
        draw_guests(&mut canvas, &park, &camera);
        assert!(canvas.commands().is_empty());
    }

    #[test]
    fn each_guest_is_a_shadow_a_body_and_a_head() {
        let (park, camera) = busy();
        let mut canvas = Recorder::new();
        draw_guests(&mut canvas, &park, &camera);

        let drawn = lines(&canvas);
        assert_eq!(drawn.len() % 3, 0, "a guest was drawn in pieces");
        assert!(!drawn.is_empty(), "the crowd was not drawn at all");
        assert!(
            drawn.len() / 3 <= park.guests().len(),
            "more people were drawn than are in the park"
        );

        for guest in drawn.chunks(3) {
            let (shadow, body, head) = (guest[0], guest[1], guest[2]);
            assert_eq!(shadow.2, SHADOW);
            assert_eq!(head.2, SKIN);
            assert!(
                (shadow.0.y - shadow.1.y).abs() < f32::EPSILON,
                "the shadow is not flat"
            );
            assert!(body.1.y < body.0.y, "the body hangs below the feet");
            assert!(head.1.y < head.0.y, "the head is under the shoulders");
            assert!(!park.guests().is_empty() && body.2 != SHADOW);
        }
    }

    #[test]
    fn the_crowd_is_drawn_back_to_front() {
        let (park, camera) = busy();
        let mut canvas = Recorder::new();
        draw_guests(&mut canvas, &park, &camera);

        let feet: Vec<f32> = lines(&canvas).chunks(3).map(|guest| guest[0].0.y).collect();
        for pair in feet.windows(2) {
            assert!(pair[0] <= pair[1], "a guest was drawn behind a nearer one");
        }
    }

    #[test]
    fn guests_off_the_edge_of_the_window_are_not_drawn() {
        let (park, mut camera) = busy();

        let mut visible = Recorder::new();
        draw_guests(&mut visible, &park, &camera);
        assert!(!visible.commands().is_empty());

        // Look somewhere the crowd is not.
        camera.look_at(TilePos::new(10_000, 10_000).centre());
        let mut elsewhere = Recorder::new();
        draw_guests(&mut elsewhere, &park, &camera);
        assert!(elsewhere.commands().is_empty(), "the cull did not fire");
    }

    #[test]
    fn zooming_in_makes_the_crowd_bigger() {
        let (park, mut camera) = busy();

        let mut close = Recorder::new();
        camera.set_zoom(2.0);
        draw_guests(&mut close, &park, &camera);
        let tall = lines(&close)[1];

        let mut far = Recorder::new();
        camera.set_zoom(1.0);
        draw_guests(&mut far, &park, &camera);
        let small = lines(&far)[1];

        assert!((tall.0.y - tall.1.y) > (small.0.y - small.1.y));
    }
}
