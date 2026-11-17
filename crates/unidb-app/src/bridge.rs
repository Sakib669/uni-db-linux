//! The one place the interface and the tokio workers meet.
//!
//! Rules from spec 0001: one shot results go to a UI sink (the relm4 `Sender`
//! implements `EventSink` in the GUI crate), streams go over a bounded channel
//! consumed on the glib side, and no GTK callback ever calls `block_on`.
//! `unidb-app` itself has no GTK dependency, so this layer is testable headless.

use std::future::Future;

use tokio::runtime::Handle;
use unidb_core::{QueryResult, UniDbError};

/// The UI side of the bridge. In the GUI crate the relm4 `Sender` implements
/// this; in tests a channel sender does.
pub trait EventSink<T>: Send + Sync + 'static {
    fn send(&self, event: T);
}

/// A stream of query events. Batches arrive in order; the last event is either
/// `Finished` or `Failed`.
#[derive(Debug)]
pub enum QueryEvent {
    Started {
        query_id: u64,
    },
    Batch {
        query_id: u64,
        rows: Vec<Vec<unidb_core::Value>>,
    },
    Finished {
        query_id: u64,
        result: QueryResult,
    },
    Failed {
        query_id: u64,
        error: UniDbError,
    },
}

/// Owns a handle to the tokio runtime created at startup.
#[derive(Clone)]
pub struct Bridge {
    handle: Handle,
}

impl Bridge {
    pub fn new(handle: Handle) -> Self {
        Self { handle }
    }

    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    /// Runs a future on the worker runtime. Results reach the UI through the
    /// sink passed in by the caller, never by blocking the caller.
    pub fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.handle.spawn(future);
    }
}
