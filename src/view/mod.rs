//! Drawing the park.
//!
//! Everything here goes through [`isogrid::render::Renderer`], so the whole
//! view can be tested against a recording renderer without opening a window.

mod guest;

use isogrid::camera::Camera;
use isogrid::iso::{ScreenPoint, TilePos};
use isogrid::render::{draw_tiles, Color, Renderer, TileShape};

use crate::park::Park;

/// The colour behind everything, where there is no park.
const SKY: Color = Color::hex(0x1B_26_33);

/// The tile the pointer is over, highlighted.
const HIGHLIGHT: Color = Color::rgba(255, 255, 255, 60);

/// Text and its drop shadow.
const TEXT: Color = Color::hex(0xF2_EF_E6);
const TEXT_SHADOW: Color = Color::rgba(0, 0, 0, 160);

/// Where the HUD sits, in pixels from the top-left.
const HUD_MARGIN: f32 = 16.0;
const HUD_LINE: f32 = 22.0;
const HUD_SIZE: f32 = 20.0;

/// Draws the whole frame: sky, land, crowd, the tile under the pointer, then
/// the HUD.
///
/// The order is the whole trick — the ground is painted back to front by the
/// engine, the guests stand on top of it, the highlight goes over them, and the
/// HUD goes over everything.
pub fn draw(canvas: &mut dyn Renderer, park: &Park, camera: &Camera, hovered: Option<TilePos>) {
    canvas.clear(SKY);
    draw_land(canvas, park, camera);
    guest::draw_guests(canvas, park, camera);

    if let Some(tile) = hovered {
        draw_highlight(canvas, camera, tile);
    }

    draw_hud(canvas, park, camera, hovered);
}

/// Paints the ground, shading tiles by depth so the land reads as a solid mass
/// rather than a flat sheet of colour.
fn draw_land(canvas: &mut dyn Renderer, park: &Park, camera: &Camera) {
    let depth = f32::from(u16::try_from(park.width() + park.height()).unwrap_or(u16::MAX));

    draw_tiles(canvas, camera, park.terrain(), |tile, terrain| {
        // A gentle gradient from the back of the map to the front: at most a
        // fifth of the brightness, which is enough to give the land shape
        // without looking like a lighting bug.
        #[allow(clippy::cast_precision_loss)]
        let along = (tile.x + tile.y) as f32 / depth;
        Some(terrain.colour().shaded(0.85 + along * 0.15))
    });
}

/// Outlines and tints the tile under the pointer.
fn draw_highlight(canvas: &mut dyn Renderer, camera: &Camera, tile: TilePos) {
    let shape = tile_shape(camera, tile);
    canvas.fill_tile(shape, HIGHLIGHT);
    canvas.stroke_tile(shape, 2.0, TEXT);
}

/// Where a tile lands on screen, at the camera's current zoom.
fn tile_shape(camera: &Camera, tile: TilePos) -> TileShape {
    TileShape {
        centre: camera.world_to_screen(tile.centre()),
        width: camera.tiles().width() * camera.zoom(),
        height: camera.tiles().height() * camera.zoom(),
    }
}

/// Draws the status lines in the corner.
fn draw_hud(canvas: &mut dyn Renderer, park: &Park, camera: &Camera, hovered: Option<TilePos>) {
    let under_pointer = hovered.map_or_else(
        || "—".to_owned(),
        |tile| format!("{:?} at {}, {}", park.terrain()[tile], tile.x, tile.y),
    );

    let lines = [
        park.name().to_owned(),
        format!("Cash: {}", park.cash()),
        format!("Guests: {}", park.guests().len()),
        park.average_happiness().map_or_else(
            || "Happiness: —".to_owned(),
            |happiness| format!("Happiness: {:.0}%", happiness * 100.0),
        ),
        format!("Tick: {}", park.tick().get()),
        format!("Zoom: {:.2}x", camera.zoom()),
        under_pointer,
    ];

    for (i, line) in lines.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let y = HUD_MARGIN + HUD_SIZE + i as f32 * HUD_LINE;
        // Drawn twice: a one-pixel shadow keeps the text readable over both the
        // pale paths and the dark water.
        canvas.text(
            line,
            ScreenPoint::new(HUD_MARGIN + 1.0, y + 1.0),
            HUD_SIZE,
            TEXT_SHADOW,
        );
        canvas.text(line, ScreenPoint::new(HUD_MARGIN, y), HUD_SIZE, TEXT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use isogrid::camera::Viewport;
    use isogrid::iso::TileSize;
    use isogrid::render::{Command, Recorder};

    fn fixture() -> (Park, Camera) {
        let park = Park::new("Test Park", 16, 16, 1).expect("a valid park");
        let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(800.0, 600.0).unwrap());
        camera.look_at(TilePos::new(8, 8).centre());
        (park, camera)
    }

    fn texts(canvas: &Recorder) -> Vec<String> {
        canvas
            .commands()
            .iter()
            .filter_map(|command| match command {
                Command::Text(text, ..) => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_frame_starts_by_clearing_the_screen() {
        let (park, camera) = fixture();
        let mut canvas = Recorder::new();
        draw(&mut canvas, &park, &camera, None);
        assert!(matches!(
            canvas.commands().first(),
            Some(Command::Clear(SKY))
        ));
    }

    #[test]
    fn the_land_is_drawn_back_to_front() {
        let (park, camera) = fixture();
        let mut canvas = Recorder::new();
        draw(&mut canvas, &park, &camera, None);

        let tiles = canvas.filled_tiles();
        assert!(!tiles.is_empty());
        for pair in tiles.windows(2) {
            assert!(pair[0].0.centre.y <= pair[1].0.centre.y);
        }
    }

    #[test]
    fn the_crowd_is_drawn_over_the_land_and_under_the_hud() {
        let mut park = Park::new("Test Park", 16, 16, 1).expect("a valid park");
        for _ in 0..600 {
            park.tick_once();
        }
        let (_, camera) = fixture();

        let mut canvas = Recorder::new();
        draw(&mut canvas, &park, &camera, None);

        let last_tile = canvas
            .commands()
            .iter()
            .rposition(|command| matches!(command, Command::FillTile(..)))
            .expect("the land is drawn");
        let first_guest = canvas
            .commands()
            .iter()
            .position(|command| matches!(command, Command::Line(..)))
            .expect("the crowd is drawn");
        let first_text = canvas
            .commands()
            .iter()
            .position(|command| matches!(command, Command::Text(..)))
            .expect("the hud is drawn");

        assert!(
            last_tile < first_guest,
            "the land was painted over the crowd"
        );
        assert!(
            first_guest < first_text,
            "the crowd was painted over the hud"
        );
    }

    #[test]
    fn the_hud_reports_the_mood_of_the_crowd_once_there_is_one() {
        let (empty, camera) = fixture();
        let mut canvas = Recorder::new();
        draw(&mut canvas, &empty, &camera, None);
        assert!(texts(&canvas).iter().any(|line| line == "Happiness: —"));

        let mut park = empty;
        for _ in 0..600 {
            park.tick_once();
        }
        let mut canvas = Recorder::new();
        draw(&mut canvas, &park, &camera, None);
        assert!(texts(&canvas)
            .iter()
            .any(|line| line.starts_with("Happiness: ") && line.ends_with('%')));
    }

    #[test]
    fn the_hud_counts_the_crowd() {
        let mut park = Park::new("Test Park", 16, 16, 1).expect("a valid park");
        for _ in 0..600 {
            park.tick_once();
        }
        let (_, camera) = fixture();

        let mut canvas = Recorder::new();
        draw(&mut canvas, &park, &camera, None);
        assert!(texts(&canvas)
            .iter()
            .any(|line| line == &format!("Guests: {}", park.guests().len())));
    }

    #[test]
    fn the_hud_reports_the_park_and_the_hovered_tile() {
        let (park, camera) = fixture();
        let mut canvas = Recorder::new();
        draw(&mut canvas, &park, &camera, Some(TilePos::new(8, 8)));

        let lines = texts(&canvas);
        assert!(lines.iter().any(|line| line.contains("Test Park")));
        assert!(lines.iter().any(|line| line.contains("Cash: 10000")));
        assert!(lines.iter().any(|line| line.contains("Path at 8, 8")));
    }

    #[test]
    fn the_hud_copes_with_nothing_under_the_pointer() {
        let (park, camera) = fixture();
        let mut canvas = Recorder::new();
        draw(&mut canvas, &park, &camera, None);
        assert!(texts(&canvas).iter().any(|line| line.contains('—')));
    }

    #[test]
    fn every_hud_line_is_drawn_twice_for_its_shadow() {
        let (park, camera) = fixture();
        let mut canvas = Recorder::new();
        draw(&mut canvas, &park, &camera, None);
        assert_eq!(texts(&canvas).len() % 2, 0);
        for pair in texts(&canvas).chunks(2) {
            assert_eq!(pair[0], pair[1], "a line and its shadow disagree");
        }
    }

    #[test]
    fn the_highlight_is_drawn_after_the_land_and_only_when_hovering() {
        let (park, camera) = fixture();

        let mut without = Recorder::new();
        draw(&mut without, &park, &camera, None);
        assert!(!without
            .commands()
            .iter()
            .any(|command| matches!(command, Command::StrokeTile(..))));

        let mut with = Recorder::new();
        draw(&mut with, &park, &camera, Some(TilePos::new(8, 8)));
        let outline = with
            .commands()
            .iter()
            .position(|command| matches!(command, Command::StrokeTile(..)))
            .expect("the hovered tile is outlined");
        let last_land = with
            .commands()
            .iter()
            .rposition(
                |command| matches!(command, Command::FillTile(_, colour) if *colour != HIGHLIGHT),
            )
            .expect("the land is drawn");
        assert!(
            outline > last_land,
            "the highlight was painted over by the land"
        );
    }

    #[test]
    fn shading_never_darkens_the_land_beyond_recognition() {
        let (park, camera) = fixture();
        let mut canvas = Recorder::new();
        draw(&mut canvas, &park, &camera, None);

        for (_, colour) in canvas.filled_tiles() {
            let brightness = u32::from(colour.r) + u32::from(colour.g) + u32::from(colour.b);
            assert!(brightness > 60, "a tile came out nearly black: {colour:?}");
        }
    }
}
