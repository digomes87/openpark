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

- The track builder. `Track` is a starting tile, a heading and a list of pieces;
  nine pieces, including a chain lift, brakes and driven track. `Track::push`
  refuses track that leaves the park's height range or crosses itself, and
  `Track::is_a_circuit` is what a ride has to satisfy before it can run.
- Ride physics. A train is a place on the layout and a speed, changed only by
  gravity, rolling resistance, the lift, the brakes and the station. `Ride::test`
  sends one round before anybody is let on and refuses a layout that stalls, and
  what it finds becomes the ride's excitement and intensity.
- Rides in the park: built piece by piece and paid for per piece, priced like
  shops, queued for, ridden and paid for as guests board. They wear out, break
  down, and wait for a mechanic — the third member of staff.
- `Needs::boredom`: the need a theme park answers. A park with nothing to ride
  empties out however well it feeds people.
- Toolbars on the number row, one per family, with the space bar walking along
  the one in hand. The arrow keys point a new ride while that tool is held.
- `view::ride` draws rails, supports under raised track, platforms and trains,
  coloured by what the ride is doing.
- `--coaster` lays the demo ride the README picture shows.

- Save and load. `save::save` and `save::load` write and read a versioned JSON
  save, written to a `.part` file and renamed into place so an interrupted save
  cannot replace a good one with half of a new one. A save from an unknown
  version is refused rather than half-read. `--save` and `--load` on the command
  line, and toolbar 8 in the game.

- Queues. `Terrain::Queue` is a path that only leads to a ride; `Queue` is the
  line waiting for one, and `queue::line_from` works out where that line stands
  from the path that was laid. Guests walk to the back, shuffle up as the front
  boards, and give up if it never moves. A line holds as many as there is path
  for it.

- Scenery: trees, flowerbeds, fountains and lamps, on toolbar 9. Paid for once,
  kept for a wage bill, and blocking the tile they stand on.
- `Park::rating` and `Park::value`. Value is what is standing on the land; rating
  is what a visitor would say — how much there is to do, how the place looks, how
  worn it is, and how the crowd inside feels. Beauty is counted around the paths
  people walk, so scenery nobody sees does nothing for it.
- Arrivals follow the rating rather than a fixed clock: a park nobody has heard
  of fills at a guest every 90 ticks, one everybody is talking about at one every
  22.

- Flat rides: a carousel, a ferris wheel, a haunted house and teacups, on
  toolbar 0. `Ride` now holds a `Layout` — track with trains, or a machine on its
  own square of land — so both kinds queue, price, wear out and break down the
  same way.
- Guests have a nerve as well as a wallet, and refuse anything rougher than they
  are brave. `RideStats::category` calls a ride gentle, thrill or extreme.

### Changed

- `Park::facilities` is now a grid of `Shop` rather than `Facility`, and
  `Park::demolish` returns the `Shop` that was standing there.
- `Guest::enjoy` takes the `Shop` being used rather than a `Facility`, so it pays
  the price on that shop's board.
- `Ride::track` returns an `Option`, since a flat ride has none, and
  `Ride::tiles` / `Ride::stations` answer for both kinds.
- Walking is shared: `Walk` holds the route and the position for both guests and
  staff, and `Guest` delegates to it.
- `Park` holds a `Land` rather than a bare terrain grid; `Park::terrain` still
  returns the ground grid.
- `--tool raise` and `--tool lower` are now `price-up` and `price-down`; `raise`
  and `dig` move the land.
