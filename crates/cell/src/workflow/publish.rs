use crate::cell::error::CellResult;
use sx_types::prelude::*;

/// Publish DHT ops based on a segment of a source chain
/// Start from the head of the provided snapshot, and continue generating DHT ops
/// and publishing them until reaching the `publish_until` address
pub async fn publish<'env>(publish_until: &Address) -> CellResult<()> {
    unimplemented!()
}
