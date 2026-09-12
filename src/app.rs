//! The game loop's other half: state, controls, and what a tick means.

use isogrid::backend::macroquad::App;
use isogrid::camera::{Camera, Viewport};
use isogrid::input::{Button, Input, Key};
use isogrid::iso::{ScreenPoint, TilePos, TileSize};
use isogrid::render::Renderer;
use isogrid::time::Tick;

use crate::park::Park;
use crate::tool::Tool;
use crate::view::{self, Overlay};

/// How fast the arrow keys scroll, in tiles per tick.
const KEYBOARD_PAN_SPEED: f32 = 0.25;

/// How much one notch of the wheel changes the zoom.
const ZOOM_STEP: f32 = 1.25;

/// How many frames to draw before taking a screenshot.
///
/// The first frame of a fresh window is drawn before the size the operating
/// system actually gave it is known, so a shot taken then can come out the
/// wrong shape.
const FRAMES_BEFORE_A_SCREENSHOT: u32 = 3;

/// The running game.
///
/// Owns the park, the camera and the pointer state. Everything that changes the
/// world happens in [`App::tick`]; [`App::draw`] only reads.
pub struct OpenPark {
    park: Park,
    camera: Camera,
    hovered: Option<TilePos>,
    /// A tile the pointer is pinned to, for screenshots taken without a hand
    /// on the mouse.
    pinned: Option<TilePos>,
    tool: Tool,
    /// What the last click did, or why it did nothing.
    status: Option<String>,
    /// Where to save a picture of the next frame but one, if this run is only
    /// here to take one.
    screenshot: Option<String>,
    /// How many frames have been drawn.
    frames: u32,
    quit: bool,
}

impl OpenPark {
    /// Starts a game looking at the middle of `park`.
    ///
    /// ```
    /// # use openpark::app::OpenPark;
    /// # use openpark::park::Park;
    /// let park = Park::new("Forest Frontiers", 32, 32, 1)?;
    /// let game = OpenPark::new(park, 1280.0, 720.0)?;
    /// assert_eq!(game.camera().zoom(), 1.0);
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Fails if the window size is not a valid viewport.
    pub fn new(park: Park, width: f32, height: f32) -> anyhow::Result<Self> {
        let mut camera = Camera::new(TileSize::CLASSIC, Viewport::new(width, height)?);
        camera.look_at(Self::centre_of(&park));
        camera.set_zoom_range(0.4, 4.0)?;

        Ok(Self {
            park,
            camera,
            hovered: None,
            pinned: None,
            tool: Tool::default(),
            status: None,
            screenshot: None,
            frames: 0,
            quit: false,
        })
    }

    /// The park being played.
    pub const fn park(&self) -> &Park {
        &self.park
    }

    /// The camera looking at it.
    pub const fn camera(&self) -> &Camera {
        &self.camera
    }

    /// The tile under the pointer, if the pointer is over the park at all.
    pub const fn hovered(&self) -> Option<TilePos> {
        self.hovered
    }

    /// What clicking would do.
    pub const fn tool(&self) -> Tool {
        self.tool
    }

    /// What the last click did, or why it did nothing.
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Picks a tool, as the space bar does.
    pub fn select(&mut self, tool: Tool) {
        self.tool = tool;
        self.status = None;
    }

    /// Asks for a picture of the game, saved to `path`, after which it closes.
    ///
    /// Taken a few frames in, once the window has settled on its real size.
    pub fn take_a_screenshot(&mut self, path: impl Into<String>) {
        self.screenshot = Some(path.into());
    }

    /// The camera, for a caller that wants to frame a shot.
    pub const fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }

    /// Pins the pointer to a tile, for a screenshot taken with nobody at the
    /// keyboard. Pass `None` to hand the pointer back to the mouse.
    pub fn pin_pointer(&mut self, tile: Option<TilePos>) {
        self.pinned = tile;
    }

    fn centre_of(park: &Park) -> isogrid::iso::GridPoint {
        #[allow(clippy::cast_precision_loss)]
        isogrid::iso::GridPoint::ground(park.width() as f32 / 2.0, park.height() as f32 / 2.0)
    }

    /// Applies one frame of input to the camera and the hover state.
    ///
    /// Split out from [`App::tick`] so it can be driven directly in tests
    /// without a window.
    pub fn handle_input(&mut self, input: &Input) {
        // Escape puts the tool down first, and only closes the game once the
        // pointer is empty — the same order every builder expects.
        if input.key_pressed(Key::Escape) {
            if self.tool.is_active() {
                self.select(Tool::Inspect);
            } else {
                self.quit = true;
            }
        }

        if input.key_pressed(Key::Space) {
            self.select(self.tool.next());
        }

        // Dragging with either the middle or the right button pans, which is
        // what every isometric game has trained people to expect.
        if input.button_down(Button::Middle) || input.button_down(Button::Right) {
            self.camera.drag(input.pointer_delta());
        }

        let mut pan = ScreenPoint::ZERO;
        if input.key_down(Key::Up) {
            pan = ScreenPoint::new(pan.x - KEYBOARD_PAN_SPEED, pan.y - KEYBOARD_PAN_SPEED);
        }
        if input.key_down(Key::Down) {
            pan = ScreenPoint::new(pan.x + KEYBOARD_PAN_SPEED, pan.y + KEYBOARD_PAN_SPEED);
        }
        if input.key_down(Key::Left) {
            pan = ScreenPoint::new(pan.x - KEYBOARD_PAN_SPEED, pan.y + KEYBOARD_PAN_SPEED);
        }
        if input.key_down(Key::Right) {
            pan = ScreenPoint::new(pan.x + KEYBOARD_PAN_SPEED, pan.y - KEYBOARD_PAN_SPEED);
        }
        self.camera.pan(pan.x, pan.y);

        match input.scroll() {
            0 => {}
            // Zoom toward the pointer, not the middle of the window.
            notches if notches > 0 => self.camera.zoom_towards(input.pointer(), ZOOM_STEP),
            _ => self.camera.zoom_towards(input.pointer(), 1.0 / ZOOM_STEP),
        }
        if input.key_pressed(Key::Plus) {
            self.camera.zoom_by(ZOOM_STEP);
        }
        if input.key_pressed(Key::Minus) {
            self.camera.zoom_by(1.0 / ZOOM_STEP);
        }

        let tile = self.camera.pick_tile(input.pointer(), 0.0);
        self.hovered = self
            .pinned
            .or_else(|| self.park.terrain().contains(tile).then_some(tile));

        if input.button_pressed(Button::Left) {
            self.use_the_tool();
        }
    }

    /// Applies the held tool to the tile under the pointer.
    fn use_the_tool(&mut self) {
        let Some(tile) = self.hovered else {
            return;
        };

        self.status = match self.tool {
            Tool::Inspect => return,
            Tool::Build(facility) => match self.park.build(tile, facility) {
                Ok(()) => Some(format!("Built a {}", facility.name().to_lowercase())),
                Err(refused) => Some(refused.to_string()),
            },
            Tool::Demolish => Some(self.park.demolish(tile).map_or_else(
                || "There is nothing there to demolish".to_owned(),
                |shop| format!("Demolished a {}", shop.kind().name().to_lowercase()),
            )),
            Tool::RaisePrice => Some(match self.park.raise_price(tile) {
                Ok(price) => format!("The price is now {price}"),
                Err(refused) => refused.to_string(),
            }),
            Tool::LowerPrice => Some(match self.park.lower_price(tile) {
                Ok(price) => format!("The price is now {price}"),
                Err(refused) => refused.to_string(),
            }),
            Tool::Hire(kind) => Some(match self.park.hire(kind) {
                Ok(_) => format!(
                    "Hired a {}, waiting at the gate",
                    kind.name().to_lowercase()
                ),
                Err(refused) => refused.to_string(),
            }),
            Tool::Fire => Some(self.park.fire_at(tile).map_or_else(
                || "There is nobody there to let go".to_owned(),
                |staff| format!("Let the {} go", staff.kind().name().to_lowercase()),
            )),
        };
    }
}

impl App for OpenPark {
    fn tick(&mut self, _tick: Tick, input: &Input) {
        self.handle_input(input);
        self.park.tick_once();
    }

    fn draw(&mut self, canvas: &mut dyn Renderer, _alpha: f32) {
        let overlay = Overlay {
            hovered: self.hovered,
            tool: self.tool,
            status: self.status.as_deref(),
        };
        view::draw(canvas, &self.park, &self.camera, &overlay);

        self.frames += 1;
        if self.frames >= FRAMES_BEFORE_A_SCREENSHOT {
            if let Some(path) = self.screenshot.take() {
                crate::screenshot::capture(&path);
                self.quit = true;
            }
        }
    }

    fn resize(&mut self, width: f32, height: f32) {
        if let Err(error) = self.camera.resize(width, height) {
            // A window can legitimately report a zero size while minimised.
            // Keeping the old viewport is better than crashing over it.
            tracing::debug!(%error, "ignoring an invalid window size");
        }
    }

    fn should_quit(&self) -> bool {
        self.quit
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;
    use crate::park::Facility;

    fn game() -> OpenPark {
        let park = Park::new("Test Park", 32, 32, 1).expect("a valid park");
        OpenPark::new(park, 800.0, 600.0).expect("a valid window")
    }

    fn frame(game: &mut OpenPark, at: ScreenPoint, setup: impl FnOnce(&mut Input)) {
        let mut input = Input::default();
        input.begin_frame(at);
        setup(&mut input);
        game.handle_input(&input);
    }

    #[test]
    fn a_new_game_looks_at_the_middle_of_the_park() {
        let game = game();
        let focus = game.camera().focus();
        assert_eq!((focus.x, focus.y), (16.0, 16.0));
    }

    #[test]
    fn the_pointer_hovers_the_tile_under_it() {
        let mut game = game();
        let centre = game.camera().viewport().centre();
        frame(&mut game, centre, |_| {});
        assert_eq!(game.hovered(), Some(TilePos::new(16, 16)));
    }

    #[test]
    fn nothing_is_hovered_outside_the_park() {
        let mut game = game();
        // Far off the left of the map, where there is no land.
        frame(&mut game, ScreenPoint::new(-10_000.0, 0.0), |_| {});
        assert_eq!(game.hovered(), None);
    }

    #[test]
    fn dragging_with_the_right_button_pans() {
        let mut game = game();
        let before = game.camera().focus();

        let mut input = Input::default();
        input.begin_frame(ScreenPoint::new(400.0, 300.0));
        input.press_button(Button::Right);
        game.handle_input(&input);

        input.begin_frame(ScreenPoint::new(500.0, 300.0));
        game.handle_input(&input);

        assert_ne!(game.camera().focus().x, before.x, "the view did not move");
    }

    #[test]
    fn moving_the_pointer_without_dragging_does_not_pan() {
        let mut game = game();
        let before = game.camera().focus();
        frame(&mut game, ScreenPoint::new(100.0, 100.0), |_| {});
        frame(&mut game, ScreenPoint::new(700.0, 500.0), |_| {});
        assert_eq!(game.camera().focus(), before);
    }

    #[test]
    fn the_wheel_zooms_toward_the_pointer() {
        let mut game = game();
        let cursor = ScreenPoint::new(700.0, 120.0);
        frame(&mut game, cursor, |_| {});
        let under_cursor = game.camera().screen_to_world(cursor, 0.0);

        frame(&mut game, cursor, |input| {
            input.scroll_by(1);
        });

        assert!(game.camera().zoom() > 1.0, "the wheel did not zoom in");
        let still_there = game.camera().screen_to_world(cursor, 0.0);
        assert!((still_there.x - under_cursor.x).abs() < 1e-2);
        assert!((still_there.y - under_cursor.y).abs() < 1e-2);
    }

    #[test]
    fn zoom_stays_within_the_range_the_game_set() {
        let mut game = game();
        for _ in 0..50 {
            frame(&mut game, ScreenPoint::new(400.0, 300.0), |input| {
                input.scroll_by(1);
            });
        }
        assert_eq!(game.camera().zoom(), 4.0);

        for _ in 0..100 {
            frame(&mut game, ScreenPoint::new(400.0, 300.0), |input| {
                input.scroll_by(-1);
            });
        }
        assert_eq!(game.camera().zoom(), 0.4);
    }

    #[test]
    fn the_arrow_keys_scroll_in_the_direction_they_point() {
        let mut game = game();
        let before = game.camera().focus();
        frame(&mut game, ScreenPoint::ZERO, |input| {
            input.press_key(Key::Right);
        });

        // "Right" on screen is +x and -y in grid space, which is the whole
        // reason this is worth a test.
        assert!(game.camera().focus().x > before.x);
        assert!(game.camera().focus().y < before.y);
    }

    #[test]
    fn opposite_keys_cancel_out() {
        let mut game = game();
        let before = game.camera().focus();
        frame(&mut game, ScreenPoint::ZERO, |input| {
            input.press_key(Key::Left);
            input.press_key(Key::Right);
        });
        assert_eq!(game.camera().focus(), before);
    }

    #[test]
    fn escape_puts_the_tool_down_before_it_closes_the_game() {
        let mut game = game();
        game.select(Tool::Build(Facility::Bench));

        frame(&mut game, ScreenPoint::ZERO, |input| {
            input.press_key(Key::Escape);
        });
        assert_eq!(game.tool(), Tool::Inspect, "the tool survived escape");
        assert!(!game.should_quit(), "the game closed with a tool in hand");

        frame(&mut game, ScreenPoint::ZERO, |input| {
            input.press_key(Key::Escape);
        });
        assert!(game.should_quit());
    }

    #[test]
    fn space_walks_through_the_tools() {
        let mut game = game();
        assert_eq!(game.tool(), Tool::Inspect);

        for expected in Tool::ORDER.iter().skip(1).chain(Some(&Tool::Inspect)) {
            frame(&mut game, ScreenPoint::ZERO, |input| {
                input.press_key(Key::Space);
            });
            assert_eq!(game.tool(), *expected);
        }
    }

    /// Clicks on one tile, wherever it happens to be on screen.
    ///
    /// The pointer is pinned rather than aimed: which pixel a tile sits under
    /// is the camera's business, and this is testing the click.
    fn click_on(game: &mut OpenPark, tile: TilePos) {
        game.pin_pointer(Some(tile));

        let mut input = Input::default();
        input.begin_frame(ScreenPoint::ZERO);
        input.press_button(Button::Left);
        game.handle_input(&input);
    }

    /// Somewhere in the park a bench could go.
    fn bare_ground(game: &OpenPark) -> TilePos {
        game.park()
            .terrain()
            .positions()
            .find(|tile| game.park().can_build(*tile, Facility::Bench))
            .expect("there is ground to build on")
    }

    #[test]
    fn looking_around_changes_nothing_however_much_it_is_clicked() {
        let mut game = game();
        let before = game.park().clone();
        let tile = bare_ground(&game);

        click_on(&mut game, tile);
        assert_eq!(game.park(), &before);
        assert_eq!(game.status(), None);
    }

    #[test]
    fn a_click_with_a_building_tool_builds_and_charges_for_it() {
        let mut game = game();
        let cash = game.park().cash();
        let tile = bare_ground(&game);

        game.select(Tool::Build(Facility::Bench));
        click_on(&mut game, tile);

        assert_eq!(game.park().facility_at(tile), Some(Facility::Bench));
        assert_eq!(game.park().cash(), cash - Facility::Bench.build_cost());
        assert!(game.status().unwrap().contains("Built"));
    }

    #[test]
    fn a_refused_build_says_why_and_changes_nothing() {
        let mut game = game();
        let tile = bare_ground(&game);
        game.select(Tool::Build(Facility::Bench));
        click_on(&mut game, tile);

        let cash = game.park().cash();
        click_on(&mut game, tile);

        assert_eq!(game.park().cash(), cash, "it was built twice");
        assert!(
            game.status().unwrap().contains("already something"),
            "unhelpful refusal: {:?}",
            game.status()
        );
    }

    #[test]
    fn demolishing_takes_down_what_is_there_and_says_so_when_nothing_is() {
        let mut game = game();
        let tile = bare_ground(&game);
        game.select(Tool::Build(Facility::Bench));
        click_on(&mut game, tile);

        game.select(Tool::Demolish);
        click_on(&mut game, tile);
        assert_eq!(game.park().facility_at(tile), None);
        assert!(game.status().unwrap().contains("Demolished"));

        click_on(&mut game, tile);
        assert!(game.status().unwrap().contains("nothing there"));
    }

    #[test]
    fn picking_a_tool_clears_what_the_last_click_said() {
        let mut game = game();
        let tile = bare_ground(&game);
        game.select(Tool::Demolish);
        click_on(&mut game, tile);
        assert!(game.status().is_some());

        game.select(Tool::Inspect);
        assert_eq!(game.status(), None);
    }

    #[test]
    fn a_click_outside_the_park_builds_nothing() {
        let mut game = game();
        game.select(Tool::Build(Facility::Bench));
        game.pin_pointer(None);

        let mut input = Input::default();
        input.begin_frame(ScreenPoint::new(-10_000.0, 0.0));
        input.press_button(Button::Left);
        game.handle_input(&input);

        assert_eq!(game.status(), None);
    }

    #[test]
    fn a_pinned_pointer_hovers_where_it_was_told_to() {
        let mut game = game();
        let tile = TilePos::new(3, 4);
        game.pin_pointer(Some(tile));

        frame(&mut game, ScreenPoint::new(-10_000.0, 0.0), |_| {});
        assert_eq!(game.hovered(), Some(tile));

        game.pin_pointer(None);
        frame(&mut game, ScreenPoint::new(-10_000.0, 0.0), |_| {});
        assert_eq!(game.hovered(), None);
    }

    #[test]
    fn a_minimised_window_does_not_crash_the_game() {
        let mut game = game();
        let before = game.camera().viewport();
        game.resize(0.0, 0.0);
        assert_eq!(
            game.camera().viewport(),
            before,
            "the viewport should be left alone"
        );

        game.resize(1024.0, 768.0);
        assert_eq!(game.camera().viewport().width(), 1024.0);
    }

    #[test]
    fn a_tick_advances_the_park() {
        let mut game = game();
        let before = game.park().tick();
        game.tick(Tick::ZERO, &Input::default());
        assert_eq!(game.park().tick().get(), before.get() + 1);
    }
}
