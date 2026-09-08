# 0001. Ship a library with a thin binary on top

**Status:** Accepted
**Date:** 2026-09-08

## Context

The obvious shape for a game is a binary crate: `main.rs`, some modules, done.
It is also the shape that makes a game hard to test, because everything
interesting ends up reachable only by starting the program and opening a window.

The rendering code is the sharp edge. Drawing is where most of the fiddly
ordering bugs live — a highlight painted under the land, a HUD drawn before the
thing it labels, a depth shade that turns a tile black at the far corner of the
map — and it is exactly the part that a binary crate cannot test.

## Decision

Make `openpark` a library crate with a thin `main.rs` on top. `main` opens a
window, builds a `Park`, and hands both to the engine's loop. Everything else —
including `view` — lives in the library.

Pair that with the engine's `render::Recorder`, a `Renderer` that records draw
calls instead of making them, so drawing is asserted on in a normal `cargo test`
run.

## Consequences

**Easier.** The drawing pass has real unit tests: the frame clears first, the
land is painted back to front, the highlight lands over the land and not under
it, the HUD reports the right numbers, shading never blacks out a tile. Camera
controls are tested by building an `Input` by hand, which caught that "right" on
screen is `+x` and `-y` in grid space. None of it needs a GPU, so it all runs in
CI on three platforms.

**Harder.** Everything the tests touch has to be `pub`, which makes the public
API larger than a game strictly needs and means thinking about what to expose.
`publish = false` in `Cargo.toml` keeps that from becoming a compatibility
promise to anyone else.

**Watch for.** The temptation to let logic drift into `main.rs` because it is
"just window setup". If `main.rs` grows past configuring and launching, the
benefit is quietly gone.

## Alternatives considered

**A plain binary crate.** Simpler, and integration tests can drive a binary —
but only from the outside, through stdout and exit codes, which tells you
nothing about whether the park was drawn in the right order.

**Screenshot tests against a real window.** They catch what unit tests cannot,
and they are slow, flaky, platform-dependent and break whenever a colour is
adjusted by one step. Worth adding later for a handful of golden frames; a poor
foundation to rely on.

**A workspace with a separate `openpark-core` library and `openpark-cli`
binary.** Enforces the boundary harder. Reserved for when the crate is large
enough to need it — right now it would be ceremony around four modules.
