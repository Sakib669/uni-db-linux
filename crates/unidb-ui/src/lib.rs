//! GTK4 and libadwaita interface. Compiled only with the `gui` feature, so the
//! rest of the workspace builds headless and in CI images without GTK headers.

#[cfg(feature = "gui")]
mod app;

/// Starts the native application loop. Blocks until the last window closes.
#[cfg(feature = "gui")]
pub fn run(service: std::sync::Arc<unidb_app::AppService>) -> Result<(), String> {
    app::run(service)
}

/// Present when the crate is built without the `gui` feature.
#[cfg(not(feature = "gui"))]
pub fn run() -> Result<(), String> {
    Err("UniDB was built without the gui feature".to_string())
}
