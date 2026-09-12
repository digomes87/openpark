# OpenPark

[![CI](https://github.com/digomes87/openpark/actions/workflows/ci.yml/badge.svg)](https://github.com/digomes87/openpark/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/digomes87/openpark/branch/main/graph/badge.svg)](https://codecov.io/gh/digomes87/openpark)
[![engine](https://img.shields.io/badge/engine-isogrid-5E8C3E)](https://github.com/digomes87/isogrid)
[![license](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

A theme park simulator in the spirit of **RollerCoaster Tycoon**, written in
Rust for the pleasure of writing it.

Chris Sawyer wrote the original almost entirely in x86 assembly. This is not
that, and it is not trying to replace [OpenRCT2](https://openrct2.org) either.
It is an attempt to build the thing properly from the ground up — a hand-written
isometric engine, a deterministic simulation, and tests for all of it.

![A park at opening time: paths, a lake, food stalls and a hundred guests walking
between them](docs/images/park.png)

## Status

Early, and playable enough to watch. Generated land, a camera you can pan and
zoom, guests who come through the gate, pay their admission, wander the paths,
get hungry and footsore, queue at a food stall or drop onto a bench — and go
home unhappy if the park has nothing to offer them. You can put up stalls and
benches yourself, take them down again, set what each one charges, hire staff to
keep the place tidy and the crowd cheerful — and run the whole thing into the
ground, because the bank closes a park that stays past its overdraft. Rides are
the next step — see the [roadmap](#roadmap).

Every guest carries a coloured pip: green when the visit is going well, red when
it is not. A park in trouble is visible from across the map before the number in
the corner says so.

Money is the other half of it. Every stall has its own price, and every guest its
own idea of what a meal is worth: charge over the odds and the stall goes
ignored, charge well over and the guests who do pay resent it. Wages and upkeep
come out of the bank every thirty seconds whether anybody turned up or not, and a
park that owes more than it can cover stops selling tickets for good.

The crowd wears its own shortcuts into the grass, and a handyman walks them back
to lawn — slower than a busy park ruins them, which is what makes hiring a second
one a decision.

![A food stall close up, with guests queuing beside it and a bench along the
path](docs/images/stall.png)

Building: pick a tool with the space bar and the tile under the pointer says
whether it would take it — green for yes, red for no — before you click.

![Build mode, with a green outline on the tile a food stall would go
on](docs/images/building.png)

## Running it

```sh
git clone https://github.com/digomes87/openpark
cd openpark
cargo run --release
```

On Linux you will need the usual windowing headers:

```sh
sudo apt-get install libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev
```

### Controls

| Input | Action |
| --- | --- |
| Right-drag or middle-drag | Pan the view |
| Arrow keys | Scroll |
| Mouse wheel | Zoom toward the cursor |
| `+` / `-` | Zoom in and out |
| `Space` | Cycle the tool: look, build, demolish, price up, price down, hire, fire |
| Left click | Use the tool on the tile under the pointer |
| `Esc` | Put the tool down, or quit when empty-handed |

Number keys for the toolbar wait on the engine growing them; the space bar is
the placeholder.

### Taking a screenshot

The pictures above are produced by the game itself, and can be reproduced
exactly:

```sh
cargo run --release -- --screenshot docs/images/park.png \
    --ticks 6000 --zoom 0.75 --focus 24,24
```

`--ticks` fast-forwards the simulation before the window opens, so a picture of
a busy afternoon does not take an afternoon to take. `--seed`, `--focus`,
`--zoom`, `--tool` and `--hover` frame the shot; `--help` lists them all.

## How it is put together

Two repositories, with the dependency pointing one way only:

- **[isogrid](https://github.com/digomes87/isogrid)** — the engine. Isometric
  projection, tile grids, camera, a deterministic fixed-timestep clock, A\*
  pathfinding, and rendering behind a trait. It knows nothing about parks.
- **openpark** — this repository. Land, money, guests, staff, and eventually
  rides and everything else that makes it a game.

The split is strict: if a type in the engine ever mentions a ride or a guest,
something has gone wrong. See [`ARCHITECTURE.md`](ARCHITECTURE.md).

This crate is a **library with a thin binary on top**, which is what lets the
simulation — and, through the engine's recording renderer, even the drawing
code — be tested without opening a window.

## Roadmap

- [x] Land, a camera, and a window to look at it through
- [x] Guests who walk the paths
- [x] Guest needs: hunger, energy, happiness
- [x] Food stalls and benches, and guests who spend money at them
- [x] Building and demolishing them yourself, with the mouse
- [x] Shops, prices, staff and a park that can go bankrupt
- [ ] The track builder and ride physics
- [ ] Terrain editing
- [ ] Save and load

## Development

```sh
just check   # fmt, clippy, tests and docs — what CI runs
just run     # play it
just test    # tests only
just cov     # coverage
```

The engine is tracked from its `main` branch until its first crates.io release.
The `version = "0.1"` constraint sits alongside the git source, so switching to
the published crate is a one-line deletion — and so that `cargo deny` does not
see a wildcard dependency.

Contributions are welcome — see [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))
- MIT license ([`LICENSE-MIT`](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.

**RollerCoaster Tycoon is a trademark of its respective owners.** This project is
an independent work, not affiliated with or endorsed by them, and contains no
original game assets or code.
