use crate::{ApiCellT, ConductorApiResult};
use async_trait::async_trait;
use sx_types::cell::CellId;


/// The methods that a concrete Conductor type must implement in order to work with
/// the two Conductor APIs
#[async_trait]
pub trait ApiConductorT: Sized + Send + Sync {
    type Cell: ApiCellT;

    fn cell_by_id(&self, cell_id: &CellId) -> ConductorApiResult<&Self::Cell>;
}
