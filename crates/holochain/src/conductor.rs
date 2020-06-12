// TODO: clean up deny's once parent is fully documented
#[deny(missing_docs)]
pub mod api;
mod cell;
pub mod compat;
#[allow(clippy::module_inception)]
mod conductor;
#[deny(missing_docs)]
pub mod config;
pub mod dna_store;
pub mod error;
#[deny(missing_docs)]
pub mod handle;
#[deny(missing_docs)]
pub mod interactive;
pub mod interface;
#[deny(missing_docs)]
pub mod manager;
#[deny(missing_docs)]
pub mod paths;
pub mod state;

pub use cell::{error::CellError, Cell};
pub use conductor::{Conductor, ConductorBuilder};
pub use handle::ConductorHandle;

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
