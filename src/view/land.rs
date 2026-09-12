//! Drawing the ground, at the height it actually stands.
//!
//! The engine's [`isogrid::render::draw_tiles`] paints a flat map: it projects
//! every tile centre at the base, which is exactly right for a park with no
//! hills in it and wrong for one with. So the land pass lives here instead, and
//! does two things the flat one cannot — it lifts each tile to its own height,
//! and it fills in the face below it where the ground drops away, so a hill
//! reads as a solid mass rather than as a diamond floating over its neighbours.

use isogrid::camera::Camera;
use isogrid::iso::{GridPoint, TilePos};
use isogrid::render::{Renderer, TileShape};

use crate::park::{Land, Park};

/// How much darker the near edge of the park is drawn than the far one.
///
/// Depth shading, so the land reads as a solid mass rather than a flat sheet of
/// colour.
const DEPTH_SHADE: f32 = 0.15;

/// How much darker the side of a step is than its top.
///
/// The one light source in the park: tops are lit, faces are not.
const FACE_SHADE: f32 = 0.5;

/// How much brighter each step up a hill is drawn.
///
/// Without it a hill of grass is the same green as the plain it stands on, and
/// the only thing saying it is a hill is a sliver of shadow along one edge.
const HEIGHT_SHEEN: f32 = 0.11;

/// Draws the whole visible park, back to front and bottom up.
pub fn draw_land(canvas: &mut dyn Renderer, park: &Park, camera: &Camera) {
    let land = park.land();

    // Tiles well below the top of the screen can still be visible once they
    // are lifted, so the cull is widened by the height of the tallest thing in
    // the park.
    let margin = u32::try_from(land.highest()).unwrap_or(0) + 1;
    let visible = camera.visible_tiles(margin);

    let reach = f32::from(u16::try_from(park.width() + park.height()).unwrap_or(u16::MAX));

    for tile in land.terrain().draw_order_within(visible) {
        let Some(terrain) = land.ground(tile) else {
            continue;
        };

        let top = land.height_at(tile).unwrap_or(Land::MIN_HEIGHT);
        let sheen = 1.0 + f32::from(top) * HEIGHT_SHEEN;
        let colour = terrain.colour().shaded(depth(tile, reach) * sheen);

        // The face first, so the lit top is drawn over the top of it.
        for step in (drops_to(land, tile)..top).rev() {
            canvas.fill_tile(
                shape(camera, tile, f32::from(step)),
                colour.shaded(FACE_SHADE),
            );
        }

        canvas.fill_tile(shape(camera, tile, f32::from(top)), colour);
    }
}

/// How far down the exposed face of `tile` goes.
///
/// Down to its lowest neighbour: anything below that is hidden behind the
/// neighbour's own face, and the edge of the map drops to the base.
fn drops_to(land: &Land, tile: TilePos) -> i16 {
    tile.neighbours()
        .iter()
        .map(|beside| land.height_at(*beside).unwrap_or(Land::MIN_HEIGHT))
        .min()
        .unwrap_or(Land::MIN_HEIGHT)
}

/// How brightly a tile is lit, by how far back in the park it sits.
fn depth(tile: TilePos, reach: f32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let distance = (tile.x + tile.y) as f32;
    1.0 - DEPTH_SHADE + DEPTH_SHADE * (distance / reach.max(1.0)).clamp(0.0, 1.0)
}

/// Where one tile lands on screen at height `z`, at the camera's zoom.
pub fn shape(camera: &Camera, tile: TilePos, z: f32) -> TileShape {
    let centre = tile.centre();
    TileShape {
        centre: camera.world_to_screen(GridPoint::new(centre.x, centre.y, z)),
        width: camera.tiles().width() * camera.zoom(),
        height: camera.tiles().height() * camera.zoom(),
    }
}

/// Where somebody part way between two tiles is, height and all.
///
/// Walking up a step is a climb rather than a hop: the height is interpolated
/// along with the position, so a guest crossing a ramp rises with it.
pub fn between(land: &Land, from: TilePos, to: Option<TilePos>, progress: f32) -> GridPoint {
    let here = from.centre();
    let up = land.elevation(from);

    let Some(to) = to else {
        return GridPoint::new(here.x, here.y, up);
    };

    let there = to.centre();
    let climb = land.elevation(to) - up;
    GridPoint::new(
        here.x + (there.x - here.x) * progress,
        here.y + (there.y - here.y) * progress,
        up + climb * progress,
    )
}

#[cfg(test)]
mod tests {
    // The heights compared below are whole steps, copied rather than computed.
    #![allow(clippy::float_cmp)]

    use super::*;
    use isogrid::camera::Viewport;
    use isogrid::iso::TileSize;
    use isogrid::render::{Command, Recorder};

    use crate::park::Terrain;

    fn fixture() -> (Park, Camera) {
        let park = Park::new("Hilly", 16, 16, 2).expect("a valid park");
        let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(1600.0, 1200.0).unwrap());
        camera.look_at(TilePos::new(8, 8).centre());
        (park, camera)
    }

    fn fills(canvas: &Recorder) -> usize {
        canvas
            .commands()
            .iter()
            .filter(|command| matches!(command, Command::FillTile(..)))
            .count()
    }

    #[test]
    fn every_tile_gets_a_top_and_a_hill_gets_a_face() {
        let (park, camera) = fixture();
        let mut canvas = Recorder::new();
        draw_land(&mut canvas, &park, &camera);

        assert!(
            fills(&canvas) > park.terrain().len(),
            "{} diamonds for {} tiles: the hills have no faces",
            fills(&canvas),
            park.terrain().len()
        );
    }

    #[test]
    fn a_raised_tile_is_drawn_with_the_face_below_it() {
        let (mut park, camera) = fixture();
        let tile = TilePos::new(8, 8);
        park.set_terrain(tile, Terrain::Grass);

        let mut before = Recorder::new();
        draw_land(&mut before, &park, &camera);

        for _ in 0..3 {
            park.raise(tile).expect("the land should take it");
        }

        let mut after = Recorder::new();
        draw_land(&mut after, &park, &camera);
        assert!(
            fills(&after) > fills(&before),
            "raising a tile drew no more of its face"
        );
    }

    #[test]
    fn a_raised_tile_is_drawn_higher_up_the_screen() {
        let (mut park, camera) = fixture();
        let tile = TilePos::new(8, 8);

        let ground = shape(&camera, tile, 0.0).centre;
        park.raise(tile).unwrap();
        let raised = shape(&camera, tile, park.land().elevation(tile)).centre;

        assert!(raised.y < ground.y, "the hill went the wrong way");
        assert!((raised.x - ground.x).abs() < f32::EPSILON, "and sideways");
    }

    #[test]
    fn nothing_off_screen_is_drawn() {
        let (park, mut camera) = fixture();
        camera.look_at(TilePos::new(-500, -500).centre());

        let mut canvas = Recorder::new();
        draw_land(&mut canvas, &park, &camera);
        assert_eq!(fills(&canvas), 0);
    }

    #[test]
    fn somebody_walking_up_a_ramp_rises_with_it() {
        let (mut park, _) = fixture();
        let (here, there) = (TilePos::new(4, 4), TilePos::new(4, 5));
        park.raise(there).unwrap();
        let land = park.land();

        let half_way = between(land, here, Some(there), 0.5);
        assert!(half_way.z > land.elevation(here));
        assert!(half_way.z < land.elevation(there));

        let standing = between(land, here, None, 0.0);
        assert_eq!(standing.z, land.elevation(here));
        assert_eq!(standing.x, here.centre().x);
    }
}
