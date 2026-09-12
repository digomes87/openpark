# Architecture

## The split

The game depends on the engine. The engine never depends on the game.

- **[isogrid](https://github.com/digomes87/isogrid)** owns everything that is
  true of any isometric tile game: the 2:1 projection, grid storage, the camera,
  the fixed-timestep clock, the seeded generator, A\* pathfinding, and the
  `Renderer` trait with its `macroquad` backend.
- **openpark** owns everything true only of a park simulator: terrain, money,
  guests, staff, rides, scenery, saves.

The rule is easy to state and easy to break by accident. The test in review is
simple: **can this type be explained without using the word "park"?** If yes, it
probably belongs in the engine. If no, it belongs here. The engine's
`path::Traversable` trait exists because the pathfinder is not allowed to know
what `Terrain::Water` is; this crate implements it and supplies the costs.

## Layout

```text
src/
├── park/          the simulation
│   ├── mod.rs         Park: the land, the money, the payroll, the tick
│   ├── terrain.rs     Terrain: what the ground is and what it costs to cross
│   ├── facility.rs    Facility: what a kind of stall or bench is like
│   ├── shop.rs        Shop: one built facility, its price and its till
│   ├── guest.rs       Guest: somebody visiting, and what they are doing about it
│   ├── needs.rs       Needs: hunger, energy and mood, and nothing else
│   ├── staff.rs       Staff: somebody on the payroll, and the job they do
│   └── walk.rs        Walk: a route and a position, shared by both of them
├── view/          drawing, entirely through isogrid::render::Renderer
│   ├── mod.rs         the frame: sky, land, hover highlight, HUD
│   ├── facility.rs    what is built, and the label for the hovered tile
│   ├── guest.rs       the crowd
│   └── staff.rs       the payroll
├── app.rs         OpenPark: state, controls, and what a tick means
├── tool.rs        Tool: what the mouse does when you click
├── cli.rs         Options: the flags, parsed by hand
├── screenshot.rs  the one deliberate reach past the Renderer trait
├── lib.rs         the library, so all of the above can be tested
└── main.rs        the window, and nothing else
```

`main.rs` is deliberately thin — it configures a window, builds a `Park`, and
hands both to the engine's loop. Everything worth testing lives in the library.

## The loop

The engine owns the loop; this crate supplies an `App`:

```text
real frame time ──▶ Clock ──▶ 0..n ticks ──▶ OpenPark::tick ──▶ Park::tick_once
                      │
                      └─ leftover fraction (alpha) ──▶ OpenPark::draw ──▶ view::draw
```

The division of labour matters:

- **`tick` changes the world.** It runs a fixed number of times per second of
  simulated time, whatever the frame rate, so anything that happens here is
  reproducible.
- **`draw` changes nothing.** It runs a variable number of times per tick, so
  anything it changed would depend on the machine's speed. It receives `alpha`
  to interpolate moving things, and it only reads.

Input is read once per frame and applied in `tick`, so a fast machine does not
pan the camera further than a slow one.

## Testing without a window

The engine's `render::Recorder` is a `Renderer` that records draw calls instead
of making them. That is what makes `view` testable: `cargo test` asserts that
the frame clears first, that the land is painted back to front, that the hover
highlight is drawn *after* the land rather than under it, that the HUD reports
the right numbers, and that the depth shading never darkens a tile to black.

None of it needs a GPU, and none of it is a screenshot test that breaks when a
colour changes by one.

`OpenPark::handle_input` is split out from `App::tick` for the same reason: an
`Input` can be built by hand, so camera controls have ordinary unit tests —
including the one that catches the classic isometric mistake, that "right" on
screen is `+x` **and** `-y` in grid space.

## Determinism

A `Park` is the entire save file, and the same park stepped the same number of
times produces the same result. It holds its own `Rng`, seeded when the park is
created, so world generation and future simulation replay exactly. The engine
guarantees the rest; see its `ARCHITECTURE.md`.

Money is a signed integer, never a float. A park can go into debt, and rounding
errors should not be a game mechanic.
