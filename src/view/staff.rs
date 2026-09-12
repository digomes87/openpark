//! Drawing the people the park pays.
//!
//! Staff are drawn like guests but squarer and in uniform, so that a crowd can
//! be read at a glance: the round heads are paying, the flat caps are being
//! paid.

use isogrid::camera::Camera;
use isogrid::iso::ScreenPoint;
use isogrid::render::Renderer;

use crate::park::{Park, Staff};

/// How tall a member of staff stands, as a fraction of a tile's width.
const HEIGHT: f32 = 0.44;

/// How wide their shoulders are drawn.
const WIDTH: f32 = 0.2;

/// How thick the cap above them is.
const CAP: f32 = 0.14;

/// Draws everybody on the payroll, back to front.
pub fn draw_staff(canvas: &mut dyn Renderer, park: &Park, camera: &Camera) {
    let scale = camera.tiles().width() * camera.zoom();

    // Sorted the same way the land is: whoever is further down the screen is
    // drawn last, so a member of staff in front of a stall stays in front.
    let mut order: Vec<&Staff> = park.staff().iter().collect();
    order.sort_by(|a, b| {
        let (a, b) = (a.tile(), b.tile());
        (a.x + a.y, a.x).cmp(&(b.x + b.y, b.x))
    });

    for member in order {
        let base = camera.world_to_screen(member.position());
        if !is_on_screen(base, camera, scale) {
            continue;
        }

        draw_one(canvas, base, scale, member);
    }
}

/// Whether somebody standing at `base` is worth drawing at all.
fn is_on_screen(base: ScreenPoint, camera: &Camera, scale: f32) -> bool {
    let viewport = camera.viewport();
    base.x >= -scale
        && base.y >= -scale
        && base.x <= viewport.width() + scale
        && base.y <= viewport.height() + scale
}

/// One member of staff: a body in uniform under a flat cap.
fn draw_one(canvas: &mut dyn Renderer, base: ScreenPoint, scale: f32, member: &Staff) {
    let colour = member.kind().colour();
    let height = HEIGHT * scale;
    let top = base.y - height;

    canvas.line(base, ScreenPoint::new(base.x, top), WIDTH * scale, colour);
    canvas.line(
        ScreenPoint::new(base.x - WIDTH * scale, top),
        ScreenPoint::new(base.x + WIDTH * scale, top),
        CAP * scale,
        colour.shaded(1.4),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use isogrid::camera::Viewport;
    use isogrid::iso::{TilePos, TileSize};
    use isogrid::render::{Command, Recorder};

    use crate::park::StaffKind;

    fn fixture() -> (Park, Camera) {
        let park = Park::new("On The Payroll", 32, 32, 4).expect("a valid park");
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
    fn a_park_with_nobody_on_the_payroll_draws_nothing() {
        let (park, camera) = fixture();
        let mut canvas = Recorder::new();
        draw_staff(&mut canvas, &park, &camera);
        assert!(canvas.commands().is_empty());
    }

    #[test]
    fn everybody_is_a_body_and_a_cap() {
        let (mut park, camera) = fixture();
        park.hire(StaffKind::Handyman).unwrap();
        park.hire(StaffKind::Entertainer).unwrap();

        let mut canvas = Recorder::new();
        draw_staff(&mut canvas, &park, &camera);
        assert_eq!(lines(&canvas), park.staff().len() * 2);
    }

    #[test]
    fn nobody_off_screen_is_drawn() {
        let (mut park, mut camera) = fixture();
        park.hire(StaffKind::Handyman).unwrap();
        camera.look_at(TilePos::new(-400, -400).centre());

        let mut canvas = Recorder::new();
        draw_staff(&mut canvas, &park, &camera);
        assert!(canvas.commands().is_empty());
    }
}
