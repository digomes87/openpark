# Run `just --list` to see every recipe.

default: check

# Everything CI runs, in the order CI runs it.
check: fmt-check clippy test doc

# Play it.
run:
    cargo run --release

# Play it with verbose logging.
debug:
    RUST_LOG=openpark=debug,isogrid=debug cargo run

# Format the workspace.
fmt:
    cargo fmt --all

# Fail if anything is unformatted.
fmt-check:
    cargo fmt --all --check

# Lint with warnings denied.
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Run unit, integration and doc tests.
test:
    cargo test --all-features --workspace

# Build the documentation, denying broken links.
doc:
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

# Coverage summary in the terminal.
cov:
    cargo llvm-cov --all-features --workspace

# Coverage as an HTML report.
cov-html:
    cargo llvm-cov --all-features --workspace --html --open

# Advisory and license audit.
audit:
    cargo deny check

# Build the browser version.
web:
    cargo build --release --target wasm32-unknown-unknown

# Point the engine dependency at a local checkout, for working across both repos.
link-engine:
    cargo add isogrid --path ../isogrid --features macroquad-backend,serde

# Point it back at the repository.
unlink-engine:
    cargo add isogrid --git https://github.com/digomes87/isogrid --features macroquad-backend,serde
