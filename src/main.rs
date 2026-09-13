//! The entry point: opens a window and runs the park.

use anyhow::{Context, Result};
use isogrid::backend::macroquad::run;
use isogrid::time::TickRate;
use macroquad::prelude::Conf;
use openpark::app::OpenPark;
use openpark::cli::Options;
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

    let options = Options::parse(std::env::args().skip(1)).map_err(|refused| {
        eprintln!("{refused}");
        refused
    })?;

    // A saved park comes back exactly as it was, so nothing else on the command
    // line that generates land applies to it.
    let mut park = match &options.load {
        Some(path) => openpark::save::load(path).context("failed to load the save")?,
        None => Park::new("Forest Frontiers", PARK_SIZE, PARK_SIZE, options.seed)
            .context("failed to lay out the starting park")?,
    };

    // Laid before the clock runs, so a fast-forward has something to queue for.
    if options.coaster {
        let id = openpark::demo::coaster(&mut park).context("failed to lay the demo coaster")?;
        tracing::info!(ride = id, "laid the demo coaster");
    }

    // Fast-forward before the window opens rather than waiting out the ticks at
    // playing speed: a screenshot of a park half an hour into its day should
    // not take half an hour to produce.
    for _ in 0..options.ticks {
        park.tick_once();
    }

    tracing::info!(
        park = park.name(),
        size = format!("{}x{}", park.width(), park.height()),
        guests = park.guests().len(),
        tick = park.tick().get(),
        "opening the gates"
    );

    if let Some(path) = &options.save {
        openpark::save::save(&park, path).context("failed to save the park")?;
    }

    #[allow(clippy::cast_precision_loss)]
    let mut game = OpenPark::new(park, WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32)
        .context("failed to set up the view")?;

    if let Some(path) = options.save.or(options.load) {
        game.save_to(path);
    }

    game.select(options.tool);
    game.pin_pointer(options.hover);
    if let Some(focus) = options.focus {
        game.camera_mut().look_at(focus.centre());
    }
    if let Some(zoom) = options.zoom {
        game.camera_mut().set_zoom(zoom);
    }
    if let Some(path) = options.screenshot {
        game.take_a_screenshot(path);
    }

    run(game, TickRate::CLASSIC).await;

    tracing::info!("the park is closed");
    Ok(())
}
