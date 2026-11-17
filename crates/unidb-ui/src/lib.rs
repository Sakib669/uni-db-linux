//! GTK4 and libadwaita interface. Compiled only with the `gui` feature, so the
//! rest of the workspace builds headless and in CI images without GTK headers.

#[cfg(feature = "gui")]
mod app;

/// Starts the native application loop. Blocks until the last window closes.
#[cfg(feature = "gui")]
pub fn run(
    handle: tokio::runtime::Handle,
    service: Result<std::sync::Arc<unidb_app::AppService>, String>,
) -> Result<(), String> {
    app::run(handle, service)
}

/// Loads the embedded design tokens. Only the accent and a few utility classes
/// are styled; surfaces and spacing come from libadwaita.
#[cfg(feature = "gui")]
pub(crate) fn load_stylesheet() {
    use relm4::gtk;
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("../resources/style.css"));
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

/// Present when the crate is built without the `gui` feature.
#[cfg(not(feature = "gui"))]
pub fn run() -> Result<(), String> {
    Err("UniDB was built without the gui feature".to_string())
}
