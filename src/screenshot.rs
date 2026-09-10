//! Saving a frame to a PNG.
//!
//! The one place the game reaches past [`isogrid`] to the window backend: there
//! is no way to ask a renderer trait for the pixels it has just drawn, and a
//! screenshot is worth the exception. Everything else still goes through
//! [`isogrid::render::Renderer`].

/// Writes what is currently on screen to `path`.
///
/// Reports the failure rather than returning it: a screenshot that cannot be
/// saved should not take the game down with it.
#[cfg(not(target_arch = "wasm32"))]
pub fn capture(path: &str) {
    let frame = macroquad::texture::get_screen_data();
    frame.export_png(path);
    tracing::info!(
        path,
        width = frame.width(),
        height = frame.height(),
        "saved a screenshot"
    );
}

/// Browsers have no filesystem to write to.
#[cfg(target_arch = "wasm32")]
pub fn capture(path: &str) {
    tracing::warn!(path, "screenshots are not supported in the browser");
}
