# Contributing to OpenPark

Thanks for taking a look. This is a hobby project built in the open; small,
well-tested pull requests are the easiest kind to merge.

## Ground rules

- **English everywhere** — code, comments, identifiers, commits, docs, issues.
- **`main` is always green.** Work on a short-lived branch (`feat/...`,
  `fix/...`, `docs/...`) and open a pull request.
- **Nothing merges without a test, a doc and a green CI run.**
- **The game does not reimplement the engine.** Anything true of any isometric
  tile game — projection, grids, cameras, ticks, pathfinding, rendering —
  belongs in [isogrid](https://github.com/digomes87/isogrid). The test in review:
  can this type be explained without using the word "park"?

## Getting set up

```sh
rustup component add rustfmt clippy
cargo install just cargo-llvm-cov cargo-deny lefthook
lefthook install   # runs fmt + clippy before each commit

# Linux also needs the windowing headers:
sudo apt-get install libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev
```

Working on the engine and the game together:

```sh
just link-engine     # point Cargo at ../isogrid
just unlink-engine   # point it back at the repository
```

## The loop

```sh
just check   # fmt, clippy, tests and docs — what CI runs
just run     # play it
just test    # tests only
just cov     # coverage report
```

## Commits

[Conventional Commits](https://www.conventionalcommits.org/), and one logical
change per commit:

```
feat(guests): add hunger and the decision to buy food

fix(view): draw the hover highlight after the land, not under it

docs(adr): record why the crate is a library with a thin binary
```

Types in use: `feat`, `fix`, `docs`, `refactor`, `test`, `perf`, `chore`, `ci`,
`build`. A breaking change gets a `!` (`feat!:`) and a `BREAKING CHANGE:` footer;
releases are cut from these by `release-plz`.

## Tests

- **Drawing code is tested too.** `isogrid::render::Recorder` records draw calls
  instead of making them, so ordering and HUD content are asserted on without a
  window. A new drawing pass without tests will be asked for them.
- Anything that touches simulation order gets a determinism test: same seed,
  same state after N ticks.
- Input handling is tested by building an `Input` by hand — see
  `app::tests`.
- Every public item carries a rustdoc example, and examples are compiled and run
  as doc tests.

## Architecture decisions

Choices that are expensive to reverse get a short record in
[`docs/adr/`](docs/adr/). Copy the newest one, bump the number, keep it under a
page. Decisions about the engine belong in
[its ADRs](https://github.com/digomes87/isogrid/tree/main/docs/adr) instead.
