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

## Status

Early. Right now you get a park to look at: generated land, a camera you can pan
and zoom, and a tile that highlights under the cursor. Guests, rides and money
that does something are the next steps — see the [roadmap](#roadmap).

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
| `Esc` | Quit |

## How it is put together

Two repositories, with the dependency pointing one way only:

- **[isogrid](https://github.com/digomes87/isogrid)** — the engine. Isometric
  projection, tile grids, camera, a deterministic fixed-timestep clock, A\*
  pathfinding, and rendering behind a trait. It knows nothing about parks.
- **openpark** — this repository. Land, money, and eventually rides, guests and
  everything that makes it a game.

The split is strict: if a type in the engine ever mentions a ride or a guest,
something has gone wrong. See [`ARCHITECTURE.md`](ARCHITECTURE.md).

This crate is a **library with a thin binary on top**, which is what lets the
simulation — and, through the engine's recording renderer, even the drawing
code — be tested without opening a window.

## Roadmap

- [x] Land, a camera, and a window to look at it through
- [ ] Guests who walk the paths
- [ ] Guest needs: hunger, happiness, energy, money
- [ ] The track builder and ride physics
- [ ] Shops, prices, staff and a park that can go bankrupt
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
