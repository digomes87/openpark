# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Shops keep their own price and their own till: two food stalls in one park can
  charge different amounts, and each records what it has taken and from how many
  customers.
- Guests have an opinion about a price. Anything charging more than they think it
  is worth is invisible to them; anything just over the odds is paid for and
  resented.
- Staff: a handyman who walks worn ground back to grass, and an entertainer who
  cheers up the crowd around them. Both are hired at the gate and cost a wage
  every bill.
- A wage bill every 1,200 ticks, covering every wage and the upkeep of everything
  standing on the land.
- Bankruptcy. A park that stays past `Park::DEBT_LIMIT` when the bill falls due
  is closed by the bank: no more tickets, nothing more to buy, and everybody
  inside heads for the gate.
- The crowd wears grass down to dirt along the routes it actually walks, and
  dislikes walking on it.
- Price, hire and fire tools on the space bar cycle, and on `--tool`.
- HUD lines for the wage bill, total takings and the staff count; the cash line
  turns red in debt, and the park's name says `BANKRUPT`.

- Terrain editing. The land has a height per tile, 0 to 16 steps: `Park::raise`,
  `Park::lower` and `Park::lay` charge for the work and refuse it under a
  building or somebody's feet. Raise, dig and four laying tools are on the space
  bar and on `--tool`.
- The shape of the land reaches the crowd: a step up costs four times a flat step
  and anything steeper is a cliff nobody walks off, both through
  `Land::step_cost`.
- A new park is generated rolling rather than flat, then smoothed so no
  generated slope is a cliff.
- `view::land` draws the ground at the height it stands, with the exposed face
  below it, and everybody — guests, staff, buildings — stands on top of it.
  Hover picking follows the hills.

### Changed

- `Park::facilities` is now a grid of `Shop` rather than `Facility`, and
  `Park::demolish` returns the `Shop` that was standing there.
- `Guest::enjoy` takes the `Shop` being used rather than a `Facility`, so it pays
  the price on that shop's board.
- Walking is shared: `Walk` holds the route and the position for both guests and
  staff, and `Guest` delegates to it.
- `Park` holds a `Land` rather than a bare terrain grid; `Park::terrain` still
  returns the ground grid.
- `--tool raise` and `--tool lower` are now `price-up` and `price-down`; `raise`
  and `dig` move the land.
