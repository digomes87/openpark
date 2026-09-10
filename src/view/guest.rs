//! Drawing the crowd.
//!
//! Guests are placeholder art like everything else: a shadow, a body, a head
//! and a pip saying how the visit is going, built out of four lines so they
//! scale with the zoom and cost nothing to draw. Sprites replace this later
//! without the rest of the view noticing.

use isogrid::camera::Camera;
use isogrid::iso::ScreenPoint;
use isogrid::render::{Color, Renderer};

use crate::park::{Guest, Park};

/// What a guest casts on the ground.
const SHADOW: Color = Color::rgba(0, 0, 0, 55);

/// The colour of a head.
const SKIN: Color = Color::hex(0xE8_C3_9E);

/// The mood pip above a guest, from delighted to miserable.
const DELIGHTED: Color = Color::hex(0x6C_D6_5F);
const CONTENT: Color = Color::hex(0xE8_D6_4A);
const MISERABLE: Color = Color::hex(0xD9_54_4D);

/// Every measurement below is a fraction of a tile's width on screen, so a
/// guest stays the same size relative to the land at any zoom.
const BODY_HEIGHT: f32 = 0.30;
const HEAD_HEIGHT: f32 = 0.09;
const BODY_WIDTH: f32 = 0.12;
const SHADOW_WIDTH: f32 = 0.24;
const PIP_HEIGHT: f32 = 0.06;
const PIP_GAP: f32 = 0.05;

/// How the head, the shadow and the mood pip are drawn against [`BODY_WIDTH`].
const HEAD_WIDTH: f32 = 0.6;
const SHADOW_THICKNESS: f32 = 0.3;
const PIP_WIDTH: f32 = 0.5;

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
        draw_guest(canvas, feet, scale, guest);
    }
}

/// The colour of the pip over a guest's head.
///
/// Green through amber to red, so a park going wrong reads as a change of
/// colour across the whole crowd rather than a number in the corner.
fn mood_colour(happiness: f32) -> Color {
    let happiness = happiness.clamp(0.0, 1.0);
    if happiness >= 0.5 {
        blend(CONTENT, DELIGHTED, (happiness - 0.5) * 2.0)
    } else {
        blend(MISERABLE, CONTENT, happiness * 2.0)
    }
}

/// Mixes two colours, `amount` of the way from `from` to `to`.
fn blend(from: Color, to: Color, amount: f32) -> Color {
    let mix = |a: u8, b: u8| {
        let (a, b) = (f32::from(a), f32::from(b));
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let mixed = (a + (b - a) * amount).clamp(0.0, 255.0) as u8;
        mixed
    };

    Color::rgba(
        mix(from.r, to.r),
        mix(from.g, to.g),
        mix(from.b, to.b),
        mix(from.a, to.a),
    )
}

/// Whether a guest standing at `feet` is worth drawing at all.
fn is_on_screen(feet: ScreenPoint, camera: &Camera, scale: f32) -> bool {
    let viewport = camera.viewport();
    feet.x >= -scale
        && feet.y >= -scale
        && feet.x <= viewport.width() + scale
        && feet.y <= viewport.height() + scale
}

/// One guest: a shadow under the feet, a body, a head, and a pip above it
/// saying how the visit is going.
fn draw_guest(canvas: &mut dyn Renderer, feet: ScreenPoint, scale: f32, guest: &Guest) {
    let half_shadow = SHADOW_WIDTH * scale / 2.0;
    canvas.line(
        ScreenPoint::new(feet.x - half_shadow, feet.y),
        ScreenPoint::new(feet.x + half_shadow, feet.y),
        BODY_WIDTH * scale * SHADOW_THICKNESS,
        SHADOW,
    );

    let shoulders = ScreenPoint::new(feet.x, feet.y - BODY_HEIGHT * scale);
    canvas.line(feet, shoulders, BODY_WIDTH * scale, guest.shirt_colour());

    let crown = ScreenPoint::new(feet.x, shoulders.y - HEAD_HEIGHT * scale);
    canvas.line(shoulders, crown, BODY_WIDTH * scale * HEAD_WIDTH, SKIN);

    let pip = ScreenPoint::new(feet.x, crown.y - PIP_GAP * scale);
    canvas.line(
        pip,
        ScreenPoint::new(pip.x, pip.y - PIP_HEIGHT * scale),
        BODY_WIDTH * scale * PIP_WIDTH,
        mood_colour(guest.needs().happiness()),
    );
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
    fn each_guest_is_a_shadow_a_body_a_head_and_a_mood() {
        let (park, camera) = busy();
        let mut canvas = Recorder::new();
        draw_guests(&mut canvas, &park, &camera);

        let drawn = lines(&canvas);
        assert_eq!(drawn.len() % 4, 0, "a guest was drawn in pieces");
        assert!(!drawn.is_empty(), "the crowd was not drawn at all");
        assert!(
            drawn.len() / 4 <= park.guests().len(),
            "more people were drawn than are in the park"
        );

        for guest in drawn.chunks(4) {
            let (shadow, body, head, pip) = (guest[0], guest[1], guest[2], guest[3]);
            assert_eq!(shadow.2, SHADOW);
            assert_eq!(head.2, SKIN);
            assert!(
                (shadow.0.y - shadow.1.y).abs() < f32::EPSILON,
                "the shadow is not flat"
            );
            assert!(body.1.y < body.0.y, "the body hangs below the feet");
            assert!(head.1.y < head.0.y, "the head is under the shoulders");
            assert!(pip.1.y < head.1.y, "the mood is not above the head");
            assert_ne!(body.2, SHADOW, "a guest was drawn in shadow");
        }
    }

    #[test]
    fn the_crowd_is_drawn_back_to_front() {
        let (park, camera) = busy();
        let mut canvas = Recorder::new();
        draw_guests(&mut canvas, &park, &camera);

        let feet: Vec<f32> = lines(&canvas).chunks(4).map(|guest| guest[0].0.y).collect();
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
    fn the_mood_pip_runs_from_red_through_amber_to_green() {
        assert_eq!(mood_colour(0.0), MISERABLE);
        assert_eq!(mood_colour(0.5), CONTENT);
        assert_eq!(mood_colour(1.0), DELIGHTED);

        // Out-of-range moods are clamped rather than wrapping to a wild colour.
        assert_eq!(mood_colour(-5.0), MISERABLE);
        assert_eq!(mood_colour(5.0), DELIGHTED);

        let glum = mood_colour(0.25);
        assert!(glum != MISERABLE && glum != CONTENT, "no blending happened");
        assert!(glum.r > CONTENT.r.min(MISERABLE.r));
    }

    #[test]
    fn a_miserable_guest_and_a_happy_one_are_told_apart_at_a_glance() {
        assert_ne!(mood_colour(0.1), mood_colour(0.9));
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
