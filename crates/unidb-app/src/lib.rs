//! Application services sit between the GTK interface and the engine and store.
//! The UI never touches a driver directly; it sends commands and receives typed
//! events through one bridge.

pub mod bridge;
pub mod service;

pub use bridge::{Bridge, EventSink, QueryEvent};
pub use service::AppService;
