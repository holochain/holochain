//! Provider trait for accessing publish triggers from cells.

use super::super::queue_consumer::TriggerSender;
use holochain_state::prelude::CellId;
use std::future::Future;
use std::pin::Pin;

/// Provider trait for retrieving publish triggers.
/// This abstracts away the conductor dependency from workflows.
#[cfg_attr(test, mockall::automock)]
pub trait PublishTriggerProvider: Send + Sync {
    /// Get the publish trigger for a cell if it exists.
    /// Returns None if the cell is not running.
    fn get_publish_trigger(&self, cell_id: &CellId) -> Pin<Box<dyn Future<Output = Option<TriggerSender>> + Send + '_>>;
}

/// Implementation of [`PublishTriggerProvider`] for [`ConductorHandle`].
impl PublishTriggerProvider for crate::conductor::ConductorHandle {
    fn get_publish_trigger(&self, cell_id: &CellId) -> Pin<Box<dyn Future<Output = Option<TriggerSender>> + Send + '_>> {
        let cell_id = cell_id.clone();
        Box::pin(async move {
            // Use get_cell_triggers which gives us the QueueTriggers for a running cell
            match self.get_cell_triggers(&cell_id).await {
                Ok(triggers) => Some(triggers.publish_dht_ops),
                Err(_) => None, // Cell not running or not found
            }
        })
    }
}
