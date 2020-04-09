mod error;
pub use error::*;

use tokio::task::JoinHandle;

pub type ManagedTaskHandle = JoinHandle<ManagedTaskResult>;
