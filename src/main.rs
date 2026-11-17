//! Thin binary: parse the flags, start logging, build the worker runtime,
//! bootstrap the app service, and hand over to the interface. All orchestration
//! lives in `unidb-app`.

use tracing_subscriber::EnvFilter;

fn main() {
    init_logging();

    if std::env::args().any(|argument| argument == "--version") {
        println!("unidb {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if let Err(error) = run_interface() {
        eprintln!("UniDB failed to start: {error}");
        std::process::exit(1);
    }
}

fn init_logging() {
    let filter = EnvFilter::try_from_env("UNIDB_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

#[cfg(feature = "gui")]
fn run_interface() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()
        .map_err(|error| format!("Could not start the worker runtime: {error}"))?;

    let startup = runtime.block_on(unidb_app::AppService::bootstrap(runtime.handle().clone()));
    let startup: Result<std::sync::Arc<unidb_app::AppService>, String> = startup
        .map(std::sync::Arc::new)
        .map_err(|error| format!("{} ({})", error.user_message(), error.code()));
    let service_for_shutdown = startup.as_ref().ok().cloned();

    let result = unidb_ui::run(runtime.handle().clone(), startup);

    if let Some(service) = service_for_shutdown {
        runtime.block_on(service.shutdown());
    }
    runtime.shutdown_background();
    result
}

#[cfg(not(feature = "gui"))]
fn run_interface() -> Result<(), String> {
    println!(
        "UniDB {} - built without the gui feature.",
        env!("CARGO_PKG_VERSION")
    );
    println!("Install the GTK development packages, then run: cargo run --features gui");
    println!("Development packages: libgtk-4-dev libadwaita-1-dev libgtksourceview-5-dev");
    Ok(())
}
