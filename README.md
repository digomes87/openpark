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

![A wooden coaster beside the crossroads: a station with a queue beside it, a
chain lift, and raised track on timber supports](docs/images/coaster.png)

Rides are built a piece at a time — station, straight, curves, slopes, a chain
lift, brakes, and driven track for the gentle ones — and a ride has to pass a
test run before anybody is let on. A layout has to earn its speed from its own
height: send a train at a hill it cannot climb and the test train stalls, and a
ride that stalls does not open. What the test run finds becomes the ride's
excitement and intensity, which is what decides both how much guests enjoy it and
how much they will pay for it.

![Trees and flowerbeds along the paths, a queue of guests filing towards a
coaster, and a park rating of 468 out of 1000](docs/images/scenery.png)

Trees, flowerbeds, fountains and lamps are worth putting up because the park is
judged on them — and judged where people actually walk, so a forest planted in a
corner nobody visits does nothing. The rating is what decides how busy the gate
gets: word of mouth is the only advertising a park has, and a park nobody has
heard of fills four times slower than one everybody is talking about.

Guests queue properly: they walk to the back of the line, shuffle up it as the
front boards, and give up in a worse mood than they joined in if it never moves.
A line holds as many people as there is queue path to stand on, so laying more of
it is what lets a popular ride hold its crowd.

Rides wear out as they run, and a worn ride breaks down. A mechanic puts it back
together; a park without one never runs that ride again.

All of it saves and loads, dice included.

The land has a shape. A new park is rolling rather than flat, and you can raise
it, dig it out and pave it yourself. A step up costs a guest four times what flat
ground does, so a crowd goes round a hill it could have climbed; two steps is a
cliff nobody walks off at all, which is either a mistake or a wall, depending on
whether you meant it.

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
| `1` – `9` | Pick a toolbar: look, build, prices, staff, land, track, rides, saves, scenery |
| `Space` | Walk along the toolbar in hand |
| Arrow keys (with the new-ride tool) | Point the ride's first piece of track |
| Left click | Use the tool on the tile under the pointer |
| `Esc` | Put the tool down, or quit when empty-handed |

The number row picks a toolbar and the space bar walks along the one in hand.
Fifteen tools on a single cycle was already too many and track pieces would have
made it twenty-five, so the engine
([isogrid#14](https://github.com/digomes87/isogrid/pull/14)) grew the number row
for it.

### Taking a screenshot

The pictures above are produced by the game itself, and can be reproduced
exactly:

```sh
cargo run --release -- --screenshot docs/images/park.png \
    --ticks 6000 --zoom 0.75 --focus 24,24
```

`--ticks` fast-forwards the simulation before the window opens, so a picture of
a busy afternoon does not take an afternoon to take. `--seed`, `--focus`,
`--zoom`, `--tool` and `--hover` frame the shot, and `--coaster` lays the demo
ride the picture above shows; `--help` lists them all.

### Saving

```sh
cargo run --release -- --save my-park.save.json     # write one, then play
cargo run --release -- --load my-park.save.json     # carry on where it left off
```

Toolbar `8` saves and loads with a click, to `openpark.save.json` unless `--save`
or `--load` named somewhere else. A save is JSON with a version on the front:
readable, diffable, and refused outright rather than half-read if it was written
by a newer build. The dice are part of it, so a loaded park carries on exactly as
the saved one would have — the same guests do the same things.

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
- [x] Terrain editing
- [x] The track builder and ride physics
- [x] Save and load

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
