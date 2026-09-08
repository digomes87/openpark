//! `openpark` is a park simulator in the spirit of `RollerCoaster` Tycoon.
//!
//! The isometric machinery — projection, grids, camera, ticks, pathfinding,
//! rendering — lives in [`isogrid`]. Everything here is the game: land, money,
//! and eventually rides and the people who queue for them.
//!
//! The crate is a library with a thin binary on top, so that the simulation can
//! be tested without a window. Drawing goes through
//! [`isogrid::render::Renderer`], which means even the view has ordinary unit
//! tests.
//!
//! ```
//! use openpark::app::OpenPark;
//! use openpark::park::{Park, Terrain};
//!
//! let park = Park::new("Forest Frontiers", 32, 32, 1)?;
//! assert_eq!(park.cash(), Park::STARTING_CASH);
//!
//! let game = OpenPark::new(park, 1280.0, 720.0)?;
//! assert_eq!(game.park().terrain()[game.camera().focus().tile()], Terrain::Path);
//! # Ok::<(), anyhow::Error>(())
//! ```

pub mod app;
pub mod park;
pub mod view;
