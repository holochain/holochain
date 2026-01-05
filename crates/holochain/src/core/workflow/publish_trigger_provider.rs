//! Provider trait for accessing publish triggers from cells.

use super::super::queue_consumer::TriggerSender;
use holochain_state::prelude::CellId;
use must_future::MustBoxFuture;

/// Provider trait for retrieving publish triggers.
///
/// This abstracts away the conductor dependency from workflows.
#[cfg_attr(test, mockall::automock)]
pub trait PublishTriggerProvider: Send + Sync {
    /// Get the publish trigger for a cell if it exists.
    ///
    /// Returns None if the cell is not running.
    fn get_publish_trigger(&self, cell_id: &CellId) -> MustBoxFuture<'_, Option<TriggerSender>>;
}

impl PublishTriggerProvider for std::sync::Arc<crate::conductor::conductor::Conductor> {
    fn get_publish_trigger(&self, cell_id: &CellId) -> MustBoxFuture<'_, Option<TriggerSender>> {
        let cell_id = cell_id.clone();
        let conductor = self.clone();
        MustBoxFuture::new(async move {
            // Use cell_by_id to get the running cell and its triggers
            match conductor.cell_by_id(&cell_id).await {
                Ok(cell) => Some(cell.publish_dht_ops_trigger()),
                Err(_) => None, // Cell not running or not found
            }
        })
    }
}
