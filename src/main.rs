//! The entry point: opens a window and runs the park.

use anyhow::{Context, Result};
use isogrid::backend::macroquad::run;
use isogrid::time::TickRate;
use macroquad::prelude::Conf;
use openpark::app::OpenPark;
use openpark::park::Park;

/// The window the game opens in.
const WINDOW_WIDTH: i32 = 1280;
const WINDOW_HEIGHT: i32 = 720;

/// The size of the park a new game starts with, in tiles.
const PARK_SIZE: u32 = 48;

fn window() -> Conf {
    Conf {
        window_title: "OpenPark".to_owned(),
        window_width: WINDOW_WIDTH,
        window_height: WINDOW_HEIGHT,
        high_dpi: true,
        window_resizable: true,
        ..Conf::default()
    }
}

#[macroquad::main(window)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "openpark=info,isogrid=warn".into()),
        )
        .init();

    // A fixed seed for now. Choosing one — a scenario, a random park, a save
    // file — is the next thing to build, and hard-coding it keeps every run
    // reproducible until then.
    let park = Park::new("Forest Frontiers", PARK_SIZE, PARK_SIZE, 1)
        .context("failed to lay out the starting park")?;

    tracing::info!(
        park = park.name(),
        size = format!("{}x{}", park.width(), park.height()),
        "opening the gates"
    );

    #[allow(clippy::cast_precision_loss)]
    let game = OpenPark::new(park, WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32)
        .context("failed to set up the view")?;

    run(game, TickRate::CLASSIC).await;

    tracing::info!("the park is closed");
    Ok(())
}
