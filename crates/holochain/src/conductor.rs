//! A Conductor manages interactions between its contained [Cell]s, as well as
//! interactions with the outside world. It is primarily a mediator of messages.
//!
//! The Conductor exposes two types of external interfaces:
//! - App interface: used by Holochain app UIs to drive the behavior of Cells,
//! - Admin interface: used to modify the Conductor itself, including adding and removing Cells
//!
//! It also exposes an internal interface to Cells themselves, allowing Cells
//! to call zome functions on other Cells, as well as to send Signals to the
//! outside world

#![deny(missing_docs)]
// TODO: clean up allows once parent is fully documented

pub mod api;
mod cell;
#[allow(clippy::module_inception)]
#[allow(missing_docs)]
mod conductor;
#[allow(missing_docs)]
pub mod config;
#[allow(missing_docs)]
pub mod dna_store;
pub mod entry_def_store;
#[allow(missing_docs)]
pub mod error;
pub mod handle;
pub mod interactive;
pub mod interface;
pub mod manager;
pub mod p2p_store;
pub mod paths;
pub mod state;

pub use cell::error::CellError;
pub use cell::Cell;
pub use conductor::Conductor;
pub use conductor::ConductorBuilder;
pub use conductor::ConductorStateDb;
pub use handle::ConductorHandle;

/// setup a tokio runtime that meets the conductor's needs
pub fn tokio_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new()
        // we use both IO and Time tokio utilities
        .enable_all()
        // we want to use multiple threads
        .threaded_scheduler()
        // we want to use thread count matching cpu count
        // (sometimes tokio by default only uses half cpu core threads)
        .core_threads(num_cpus::get())
        // give our threads a descriptive name (they'll be numbered too)
        .thread_name("holochain-tokio-thread")
        // build the runtime
        .build()
        // panic if we cannot (we cannot run without it)
        .expect("can build tokio runtime")
}
